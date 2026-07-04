use solana_pubkey::Pubkey;
use solana_transaction_status::{
    TransactionTokenBalance, UiTransactionTokenBalance, option_serializer::OptionSerializer,
};
use spl_token_interface::native_mint;
use std::{
    collections::{HashMap, HashSet},
    num::TryFromIntError,
    ops::Mul,
    str::FromStr,
    sync::Arc,
};
use thiserror::Error;

use crate::{transaction::TxMetadata, util::keys::AccountKeys};

#[derive(Clone)]
pub struct TxBalance {
    keys: Arc<AccountKeys>,
    meta: Arc<TxMetadata>,
}

pub enum Addr {
    Index(usize),
    Key(Pubkey),
}

impl TxBalance {
    pub fn new(meta: Arc<TxMetadata>, keys: Arc<AccountKeys>) -> Self {
        Self { keys, meta }
    }

    pub fn find_native_balance(&self, addr: Addr) -> NativeBalance {
        let idx = match addr {
            Addr::Index(idx) => idx,
            Addr::Key(key) => match self.find_index_for_wallet(&key) {
                Some(idx) => idx,
                None => return NativeBalance::default(),
            },
        };

        self.find_native_balance_by_idx(idx)
    }

    fn find_native_balance_by_idx(&self, idx: usize) -> NativeBalance {
        NativeBalance {
            before: self.meta.pre_balances.get(idx).copied().unwrap_or(0),
            after: self.meta.post_balances.get(idx).copied().unwrap_or(0),
        }
    }

    pub fn find_decimals_for_mint(&self, mint: &Pubkey) -> Option<u8> {
        if *mint == native_mint::ID {
            return Some(native_mint::DECIMALS);
        }

        let map_fn = |bal: &TransactionTokenBalance| -> Option<u8> {
            (Pubkey::from_str(&bal.mint).ok()? == *mint).then_some(bal.ui_token_amount.decimals)
        };

        self.meta
            .pre_token_balances
            .iter()
            .find_map(map_fn)
            .or_else(|| self.meta.post_token_balances.iter().find_map(map_fn))
    }

    pub fn find_token_account_balance(&'_ self, addr: Addr) -> TokenBalance {
        let idx = match addr {
            Addr::Index(idx) => idx,
            Addr::Key(key) => match self.find_index_for_wallet(&key) {
                Some(idx) => idx,
                None => return TokenBalance::default(),
            },
        };

        self.find_token_account_balance_by_idx(idx)
    }

    fn find_token_account_balance_by_idx(&'_ self, idx: usize) -> TokenBalance {
        let find_balance = |balances: &[TransactionTokenBalance], idx: usize| -> Option<Balance> {
            balances.iter().find_map(|bal| {
                if (bal.account_index as usize) == idx {
                    Some(Balance::from(bal.clone()))
                } else {
                    None
                }
            })
        };

        TokenBalance {
            before: find_balance(&self.meta.pre_token_balances, idx),
            after: find_balance(&self.meta.post_token_balances, idx),
        }
    }

    fn find_index_for_wallet(&self, key: &Pubkey) -> Option<usize> {
        self.keys
            .iter()
            .enumerate()
            .find_map(|(i, k)| if k == key { Some(i) } else { None })
    }

    pub fn pre_token_balances(&self, owner: &Pubkey, mint: &Pubkey) -> Vec<Balance> {
        Self::token_balances(&self.meta.pre_token_balances, owner, mint)
    }

    pub fn post_token_balances(&self, owner: &Pubkey, mint: &Pubkey) -> Vec<Balance> {
        Self::token_balances(&self.meta.post_token_balances, owner, mint)
    }

    pub fn decimals_map(&self) -> HashMap<&str, u8> {
        let mut map = HashMap::new();
        let mut idxs = HashSet::new();

        for b in self.meta.pre_token_balances.iter() {
            if idxs.contains(&b.account_index) {
                continue;
            }

            map.insert(b.mint.as_str(), b.ui_token_amount.decimals);
            idxs.insert(b.account_index);
        }

        for b in self.meta.post_token_balances.iter() {
            if idxs.contains(&b.account_index) {
                continue;
            }

            map.insert(b.mint.as_str(), b.ui_token_amount.decimals);
            idxs.insert(b.account_index);
        }

        map
    }

    fn token_balances(
        balances: &[TransactionTokenBalance],
        owner: &Pubkey,
        mint: &Pubkey,
    ) -> Vec<Balance> {
        balances
            .iter()
            .filter_map(|b| {
                let b_owner = Pubkey::from_str(&b.owner).ok()?;
                let b_mint = Pubkey::from_str(&b.mint).ok()?;
                (b_owner == *owner && b_mint == *mint).then_some(Balance::from(b.clone()))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Balance(TransactionTokenBalance);

impl std::ops::Deref for Balance {
    type Target = TransactionTokenBalance;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<TransactionTokenBalance> for Balance {
    fn from(value: TransactionTokenBalance) -> Self {
        Self(value)
    }
}

impl From<UiTransactionTokenBalance> for Balance {
    fn from(value: UiTransactionTokenBalance) -> Self {
        Self(map_ui_balance(value))
    }
}

fn map_ui_balance(value: UiTransactionTokenBalance) -> TransactionTokenBalance {
    TransactionTokenBalance {
        account_index: value.account_index,
        mint: value.mint,
        ui_token_amount: value.ui_token_amount,
        owner: match value.owner {
            OptionSerializer::Some(program) => program,
            _ => Pubkey::default().to_string(),
        },
        program_id: match value.program_id {
            OptionSerializer::Some(program) => program,
            _ => Pubkey::default().to_string(),
        },
    }
}

impl Balance {
    pub fn ui_amount(&self) -> Option<f64> {
        self.ui_token_amount.ui_amount
    }

    pub fn amount(&self) -> u64 {
        derive_amount(self)
    }
}

/// Token account balance before and after a transaction
#[derive(Debug, Default, Clone)]
pub struct TokenBalance {
    pub before: Option<Balance>,
    pub after: Option<Balance>,
}

#[derive(Debug, Error)]
pub enum BalanceError {
    #[error("None of pre or post balance present")]
    NoBalance,
    #[error("Arithmetic overflow")]
    Overflow,
    #[error("Failed converting to I64: {0}")]
    I64Convert(#[from] TryFromIntError),
}

impl TokenBalance {
    pub fn difference(&self) -> Result<i64, BalanceError> {
        if self.before.is_none() && self.after.is_none() {
            return Err(BalanceError::NoBalance);
        }
        let before = i64::try_from(self.before.as_ref().map(|b| b.amount()).unwrap_or(0))?;
        let after = i64::try_from(self.after.as_ref().map(|b| b.amount()).unwrap_or(0))?;

        after.checked_sub(before).ok_or(BalanceError::Overflow)
    }

    pub fn positive_balance(&self) -> Option<u64> {
        let diff = self.difference().ok()?;
        diff.is_positive().then_some(diff.unsigned_abs())
    }

    pub fn negative_balance(&self) -> Option<u64> {
        let diff = self.difference().ok()?;
        diff.is_negative().then_some(diff.unsigned_abs())
    }

    pub fn mint(&self) -> Option<Pubkey> {
        let mint_str = self
            .before
            .as_ref()
            .map(|b| b.mint.clone())
            .or_else(|| self.after.as_ref().map(|b| b.mint.clone()))?;
        Pubkey::from_str(&mint_str).ok()
    }

    pub fn token_program(&self) -> Option<Pubkey> {
        self.before
            .as_ref()
            .and_then(|b| Pubkey::from_str(&b.program_id).ok())
            .or_else(|| {
                self.after
                    .as_ref()
                    .and_then(|b| Pubkey::from_str(&b.program_id).ok())
            })
    }
}

/// Lamports balance before and after a transaction
#[derive(Debug, Default, Clone)]
pub struct NativeBalance {
    pub before: u64,
    pub after: u64,
}

impl NativeBalance {
    pub fn difference(&self) -> Result<i64, BalanceError> {
        let before = i64::try_from(self.before)?;
        let after = i64::try_from(self.after)?;

        after.checked_sub(before).ok_or(BalanceError::Overflow)
    }

    pub fn positive_balance(&self) -> Option<u64> {
        let diff = self.difference().ok()?;
        diff.is_positive().then_some(diff.unsigned_abs())
    }

    pub fn negative_balance(&self) -> Option<u64> {
        let diff = self.difference().ok()?;
        diff.is_negative().then_some(diff.unsigned_abs())
    }
}

pub fn derive_amount(balance: &TransactionTokenBalance) -> u64 {
    let multiplier = 10u64.pow(balance.ui_token_amount.decimals as u32) as f64;
    balance
        .ui_token_amount
        .ui_amount
        .unwrap_or(0.0)
        .mul(multiplier)
        .trunc() as u64
}
