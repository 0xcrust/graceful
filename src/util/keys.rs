use std::sync::Arc;

pub use crate::transaction::keys::AccountKeys;

use solana_pubkey::Pubkey;
use solana_transaction::CompiledInstruction;

/// The ordered list of accounts one instruction references, resolved
/// against a transaction's full account key table (`AccountKeys`).
///
/// Indices inside an instruction (`CompiledInstruction::accounts`) are
/// positions into the *transaction's* key table, not a self-contained list.
///
/// This type does that resolution and hands accounts out one at a time by
/// an `Iterator`, matching the order a program would read them in.
pub struct InstructionAccounts {
    keys: Arc<AccountKeys>,
    indices: Arc<Vec<u8>>,
    cursor: usize,
    program_id: Pubkey,
}

impl InstructionAccounts {
    pub fn new(
        keys: Arc<AccountKeys>,
        indices: Arc<Vec<u8>>,
        program_id_idx: u8,
    ) -> Result<Self, KeyError> {
        let program_id =
            *keys
                .get(program_id_idx as usize)
                .ok_or(KeyError::InvalidProgramIdIndex {
                    index: program_id_idx,
                    table_len: keys.len(),
                })?;

        Ok(Self {
            keys,
            indices,
            cursor: 0,
            program_id,
        })
    }

    pub fn from_compiled_instruction(
        keys: Arc<AccountKeys>,
        instruction: &CompiledInstruction,
    ) -> Result<Self, KeyError> {
        Self::new(
            keys,
            Arc::new(instruction.accounts.clone()),
            instruction.program_id_index,
        )
    }

    /// The program this instruction targets.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// Total number of account slots this instruction declares.
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Number of accounts not yet consumed via `next()`/`next_account()`.
    pub fn remaining_len(&self) -> usize {
        self.indices.len() - self.cursor
    }

    /// Read the next account, erroring (rather than panicking or silently
    /// stopping) if the instruction has no more accounts.
    ///
    /// Prefer `Iterator::next()` if you want normal end-of-accounts to be a
    /// `None` rather than an error.
    pub fn next_address(&mut self) -> Result<Pubkey, KeyError> {
        self.next().ok_or(KeyError::AccountsExhausted {
            declared: self.indices.len(),
        })?
    }

    /// Anchor's convention for an omitted optional account is to pass the
    /// *program's own id* in that slot rather than leaving it out.
    /// This reads the next account and maps that sentinel to `None`.
    pub fn next_optional_account(&mut self) -> Result<Option<Pubkey>, KeyError> {
        let account = self.next_address()?;
        Ok((account != self.program_id).then_some(account))
    }

    /// Consume and return every account not yet read.
    pub fn remaining_accounts(&mut self) -> Result<Vec<Pubkey>, KeyError> {
        self.by_ref().collect()
    }

    /// Look at the account `offset` positions ahead of the cursor without consuming it.
    pub fn peek(&self, offset: usize) -> Result<Pubkey, KeyError> {
        let pos = self.cursor + offset;
        let table_idx = *self.indices.get(pos).ok_or(KeyError::AccountsExhausted {
            declared: self.indices.len(),
        })? as usize;
        self.keys
            .get(table_idx)
            .copied()
            .ok_or(KeyError::AccountIndexOutOfRange {
                index: table_idx,
                table_len: self.keys.len(),
            })
    }
}

impl Iterator for InstructionAccounts {
    type Item = Result<Pubkey, KeyError>;

    fn next(&mut self) -> Option<Self::Item> {
        let table_idx = *self.indices.get(self.cursor)? as usize;
        self.cursor += 1;

        match self.keys.get(table_idx) {
            Some(pubkey) => Some(Ok(*pubkey)),
            None => Some(Err(KeyError::AccountIndexOutOfRange {
                index: table_idx,
                table_len: self.keys.len(),
            })),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("program_id_index {index} is out of range for an account table of {table_len} entries")]
    InvalidProgramIdIndex { index: u8, table_len: usize },

    #[error("instruction has no more accounts to read (it declares {declared} total)")]
    AccountsExhausted { declared: usize },

    #[error("account index {index} is out of range for an account table of {table_len} entries")]
    AccountIndexOutOfRange { index: usize, table_len: usize },
}
