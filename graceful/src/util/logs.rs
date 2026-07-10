use crate::transaction::{
    TxMetadata,
    instruction::{Path, SolanaInstruction},
    keys::AccountKeys,
};
use std::{borrow::Cow, sync::Arc};

use once_cell::sync::OnceCell;
use solana_log_parser::{InstructionCall, ParseError, Parser};
use solana_pubkey::Pubkey;

const EVENT_IX_TAG_LE: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

#[derive(Clone)]
pub struct TxLogs {
    meta: Arc<TxMetadata>,
    parsed: Arc<OnceCell<ParsedLogs>>,
}

type InstructionCalls<'a> = Vec<InstructionCall<'a>>;

self_cell::self_cell!(
    struct ParsedLogs {
        owner: Arc<TxMetadata>,
        #[covariant]
        dependent: InstructionCalls,
    }
);

#[derive(Debug, thiserror::Error)]
pub enum TxLogsError {
    #[error("Failed to trace instruction: path: {:?}", path)]
    FailedIxTrace { path: Path },
    #[error("Error parsing logs: {0}")]
    Parse(#[from] ParseError),
    #[error("No logs for transaction")]
    NoLogs,
}

impl TxLogs {
    pub fn new(meta: Arc<TxMetadata>) -> Self {
        Self {
            meta,
            parsed: Arc::new(OnceCell::new()),
        }
    }

    pub fn program_logs<'a>(
        &'a self,
        path: &Path,
    ) -> Result<impl Iterator<Item = &'a str>, TxLogsError> {
        self.load_logs()?;
        self.get(path)
    }

    pub fn stack<'a>(&'a self) -> Result<&'a [InstructionCall<'a>], TxLogsError> {
        self.load_logs()?;
        Ok(self.instructions().expect("loaded"))
    }

    fn is_loaded(&self) -> bool {
        self.parsed.get().is_some()
    }

    fn load_logs(&self) -> Result<(), TxLogsError> {
        if self.is_loaded() {
            return Ok(());
        }

        let meta = self.meta.clone();
        self.parsed.get_or_try_init(|| {
            ParsedLogs::try_new(meta, |meta| {
                if meta.log_messages.is_empty() {
                    return Err(TxLogsError::NoLogs);
                }
                Ok(Parser.parse(&meta.log_messages)?)
            })
        })?;

        Ok(())
    }

    fn instructions(&self) -> Option<&[InstructionCall<'_>]> {
        self.parsed
            .get()
            .map(|cell| cell.borrow_dependent().as_slice())
    }

    fn get<'a>(&'a self, path: &Path) -> Result<impl Iterator<Item = &'a str>, TxLogsError> {
        let cell = self.parsed.get().expect("loaded");
        let logs = &cell.borrow_owner().log_messages;
        let stack = cell.borrow_dependent();

        let target =
            trace_ix_call(stack, path).ok_or(TxLogsError::FailedIxTrace { path: path.clone() })?;

        Ok(target.program_logs(logs))
    }
}

fn trace_ix_call<'a>(
    ixs: &'a [InstructionCall<'a>],
    path: &Path,
) -> Option<&'a InstructionCall<'a>> {
    let mut iter = path.iter();
    let first = iter.next()?;

    let mut curr = ixs.get(*first as usize)?;
    for idx in iter {
        curr = curr.invocations().nth(*idx as usize)?;
    }

    Some(curr)
}

pub fn filter_cpi_logs_data<'a, T: SolanaInstruction + 'a>(
    account_keys: &AccountKeys,
    program: &Pubkey,
    ixs: impl Iterator<Item = &'a T> + Clone,
) -> impl Iterator<Item = Cow<'a, [u8]>> + Clone {
    ixs.filter_map(move |ix| {
        let ix_program = account_keys.get(ix.program_id() as usize)?;

        // - https://github.com/coral-xyz/anchor/issues/2408#issuecomment-1447243011
        // - https://github.com/ngundotra/anchor/blob/22b902a06b5af80606439d6fe3b79ea90ddf7073/lang/attribute/event/src/lib.rs#L103
        if ix_program != program {
            return None;
        }

        let data = ix.data().ok()?;

        anchor_event_cpi_filter(data)
    })
}

fn anchor_event_cpi_filter(data: Cow<'_, [u8]>) -> Option<Cow<'_, [u8]>> {
    let len = EVENT_IX_TAG_LE.len();
    if data.len() <= len {
        return None;
    }
    if data[..len] == EVENT_IX_TAG_LE {
        Some(match data {
            Cow::Borrowed(data) => Cow::Borrowed(&data[len..]),
            Cow::Owned(mut data) => Cow::Owned(data.split_off(len)),
        })
    } else {
        None
    }
}
