pub mod aggregator;
pub mod dex;
pub mod full;
pub mod util;

use crate::{
    parse::aggregator::AggregatorSwap,
    util::{accounts::AccountKeys, balances::TxBalance, logs::TxLogs, transfer::TokenParseError},
};

use std::fmt;

use solana_pubkey::Pubkey;

use util::SwapInfoError;

use crate::{
    swap::{Program, Swap},
    transaction::instruction::{Path, SolanaInstruction},
    util::{accounts::KeyError, balances::BalanceError, logs::TxLogsError},
};

pub struct IxView<'a, T> {
    ix: &'a T,
    keys: AccountKeys,
    logs: TxLogs,
    balance: TxBalance,
}

impl<'a, T: SolanaInstruction> IxView<'a, T> {
    fn clone_new_from<'b>(ix: &'b T, from: &IxView<'a, T>) -> IxView<'b, T> {
        IxView {
            ix,
            keys: from.keys.clone(),
            logs: from.logs.clone(),
            balance: from.balance.clone(),
        }
    }

    fn program_id(&self) -> Result<Pubkey, ParseError> {
        self.keys
            .get(self.ix.accounts()[self.ix.program_id() as usize])
            .copied()
            .ok_or(ParseError::NoProgramId)
    }

    fn program(&self) -> Result<Program, ParseError> {
        Ok(Program::from(self.program_id()?))
    }
}

pub fn parse_instruction<T: SolanaInstruction>(
    view: IxView<T>,
) -> Result<Option<Parsed>, WithTrace<ParseError>> {
    with_trace(view, internal)
}

fn internal<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<Parsed>, ParseError> {
    let program = view.program()?;

    Ok(if aggregator::is_aggregator(&program) {
        aggregator::parse(view, &program)?.map(Parsed::Aggregator)
    } else if dex::is_amm(&program) {
        dex::parse(view, &program)?.map(Parsed::Dex)
    } else {
        None
    })
}

#[derive(Debug, Clone)]
pub enum Parsed {
    /// An aggregator.
    Aggregator(AggregatorSwap),
    /// A dex.
    Dex(DexSwap),
}

#[derive(Debug, Clone)]
pub struct DexSwap {
    pub user: Pubkey,
    pub market: Pubkey,
    pub program: Program,
    pub swap: Swap,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),

    #[error("Failed key transversal: {0}")]
    KeyTransversal(#[from] KeyError),

    #[error("Error deserializing borsh data: {0}")]
    BorshDeserialize(#[from] std::io::Error),

    #[error("Error decoding base64 program logs: {0}")]
    ProgramLogsDecode(base64::DecodeError),

    #[error("Failed getting tx logs: {0}")]
    TxLogs(#[from] TxLogsError),

    #[error("Failed to get program ID for instruction")]
    NoProgramId,

    #[error("Balance error: {0}")]
    Balance(#[from] BalanceError),

    #[error("Swap info error: {0}")]
    GetSwapInfo(#[from] SwapInfoError),

    #[error("No details found for swap instruction")]
    NoDetailsFound,

    #[error(transparent)]
    TokenParse(#[from] TokenParseError),

    #[error(transparent)]
    Any(#[from] anyhow::Error),
}

pub(super) fn with_trace<T: SolanaInstruction, E, R>(
    view: IxView<T>,
    f: impl Fn(IxView<T>) -> Result<Option<R>, E>,
) -> Result<Option<R>, WithTrace<E>> {
    let path = view.ix.path().clone();
    let program = view.keys.get(view.ix.program_id()).map(Program::from);
    f(view).map_err(|error| WithTrace {
        path,
        program,
        error,
    })
}

#[derive(Debug)]
pub struct WithTrace<E> {
    path: Path,
    program: Option<Program>,
    error: E,
}

impl<E: fmt::Display> fmt::Display for WithTrace<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.program {
            Some(program) => write!(
                f,
                "{}. program: {}. path: {}",
                self.error, program, self.path
            ),
            None => write!(f, "{}. program: unknown. path: {}", self.error, self.path),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for WithTrace<E> {}
