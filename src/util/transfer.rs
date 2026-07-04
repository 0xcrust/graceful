use std::sync::Arc;

use solana_bincode::limited_deserialize;
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use solana_system_interface::instruction::SystemInstruction;
use solana_transaction::InstructionError;
use spl_token_interface::instruction::TokenInstruction;

use crate::transaction::instruction::SolanaInstruction;
use crate::util::keys::{AccountKeys, InstructionAccounts, KeyError};

#[derive(Clone, Copy, Debug)]
pub enum Transfer {
    /// SOL transfer
    Native(NativeTransfer),

    /// Token transfer
    Token(TokenTransfer),
}

#[derive(Clone, Copy, Debug)]
pub struct TokenTransfer {
    /// The transfer source.
    pub source_account: Pubkey,
    /// The token mint. This is `Some` only for `TransferChecked`.
    pub mint: Option<Pubkey>,
    /// The transfer destination.
    pub destination_account: Pubkey,
    /// The source account's owner/delegate.
    pub signer: Pubkey,
    /// The amount.
    pub amount: u64,
    /// Whether this is a token-2022 action.
    pub token_22: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeTransfer {
    /// The source account.
    pub source_account: Pubkey,
    /// The destination account.
    pub destination_account: Pubkey,
    /// This is `Some` when `TransferWithSeed` was called and the source-account isn't the signer.
    pub signer: Option<Pubkey>,
    /// The transfer lamports.
    pub lamports: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenParseError {
    #[error("Missing program id at index {0}")]
    MissingProgramId(u8),

    #[error("Failed to deserialize instruction: {0}")]
    InstructionDeserialize(#[from] InstructionError),

    #[error("Error transversing key iterator: {0}")]
    KeyTransversal(#[from] KeyError),

    #[error("Error unpacking instruction: {0}")]
    InstructionUnpack(#[from] ProgramError),

    #[error("Error getting instruction data: {0}")]
    InstructionData(#[from] Box<dyn std::error::Error>),
}

pub fn parse_transfer(
    account_keys: Arc<AccountKeys>,
    accounts: Arc<Vec<u8>>,
    data: &[u8],
    program_id_index: u8,
) -> Result<Option<Transfer>, TokenParseError> {
    let id = account_keys
        .get(program_id_index as usize)
        .ok_or(TokenParseError::MissingProgramId(program_id_index))?;

    match *id {
        solana_sdk_ids::system_program::ID => {
            Ok(
                parse_native_transfer(account_keys, accounts, data, program_id_index)?
                    .map(Transfer::Native),
            )
        }
        spl_token_interface::ID | spl_token_2022_interface::ID => {
            Ok(
                parse_token_transfer(account_keys, accounts, data, program_id_index)?
                    .map(Transfer::Token),
            )
        }
        _ => Ok(None),
    }
}

pub fn parse_native_transfer(
    account_keys: Arc<AccountKeys>,
    accounts: Arc<Vec<u8>>,
    data: &[u8],
    program_id_index: u8,
) -> Result<Option<NativeTransfer>, TokenParseError> {
    let mut keys = InstructionAccounts::new(account_keys, accounts, program_id_index)?;
    let instruction = limited_deserialize::<SystemInstruction>(
        data,
        std::mem::size_of::<SystemInstruction>() as u64,
    )?;
    let lamports = match instruction {
        SystemInstruction::Transfer { lamports } => lamports,
        SystemInstruction::TransferWithSeed {
            lamports,
            from_seed: _,
            from_owner: _,
        } => lamports,
        _ => return Ok(None),
    };

    let source_account = keys.next_address()?;
    let (signer, destination_account) = if matches!(
        instruction,
        SystemInstruction::TransferWithSeed {
            lamports: _,
            from_seed: _,
            from_owner: _
        }
    ) {
        (Some(keys.next_address()?), keys.next_address()?)
    } else {
        (None, keys.next_address()?)
    };

    Ok(Some(NativeTransfer {
        source_account,
        destination_account,
        signer,
        lamports,
    }))
}

pub fn parse_token_transfer(
    account_keys: Arc<AccountKeys>,
    accounts: Arc<Vec<u8>>,
    data: &[u8],
    program_id_index: u8,
) -> Result<Option<TokenTransfer>, TokenParseError> {
    let id = *account_keys
        .get(program_id_index as usize)
        .ok_or(TokenParseError::MissingProgramId(program_id_index))?;

    if id != spl_token_interface::ID && id != spl_token_2022_interface::ID {
        return Ok(None);
    }

    if data.is_empty() || (data[0] != 3 && data[0] != 12) {
        return Ok(None);
    }

    let decoded = TokenInstruction::unpack(data)?;
    let amount = match decoded {
        TokenInstruction::Transfer { amount } => amount,
        TokenInstruction::TransferChecked {
            amount,
            decimals: _,
        } => amount,
        _ => return Ok(None),
    };

    let keys = InstructionAccounts::new(account_keys, accounts.clone(), program_id_index)?;
    let source_account = keys.peek(0)?;
    let (mint, destination_account, signer) = if matches!(
        decoded,
        TokenInstruction::TransferChecked {
            amount: _,
            decimals: _
        }
    ) {
        (Some(keys.peek(1)?), keys.peek(2)?, keys.peek(3)?)
    } else {
        (None, keys.peek(1)?, keys.peek(2)?)
    };

    Ok(Some(TokenTransfer {
        source_account,
        mint,
        destination_account,
        signer,
        amount,
        token_22: id == spl_token_2022_interface::ID,
    }))
}

pub fn parse_multiple_token_transfers(
    account_keys: &Arc<AccountKeys>,
    ixs: &[&impl SolanaInstruction],
) -> Result<Vec<TokenTransfer>, TokenParseError> {
    let mut transfers = vec![];

    for i in ixs {
        if let Some(tt) = parse_token_transfer(
            account_keys.clone(),
            i.accounts(),
            &i.data()?,
            i.program_id(),
        )? {
            transfers.push(tt);
        }
    }

    Ok(transfers)
}
