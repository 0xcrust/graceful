use std::borrow::Cow;

use solana_pubkey::Pubkey;
use solana_transaction_status::UiTransactionStatusMeta;

use crate::transaction::{instruction::SolanaInstruction, keys::AccountKeys};

const EVENT_IX_TAG_LE: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

pub fn get_program_logs(meta: &UiTransactionStatusMeta) -> Vec<String> {
    get_encoded_logs(meta)
}

pub fn filter_cpi_logs_data<'a, T: SolanaInstruction + 'a>(
    account_keys: &AccountKeys,
    program: &Pubkey,
    ixs: impl Iterator<Item = &'a T>,
) -> impl Iterator<Item = Cow<'a, [u8]>> {
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

fn get_encoded_logs(meta: &UiTransactionStatusMeta) -> Vec<String> {
    Option::<Vec<String>>::from(meta.log_messages.clone()).unwrap_or_default()
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
