use std::sync::Arc;

use solana_message::v0::LoadedAddresses;
use solana_pubkey::Pubkey;

/// The keys of the accounts involved in a transaction.
#[derive(Debug, Default)]
pub struct AccountKeys {
    /// Account keys submitted directly with the transaction.
    static_keys: Vec<Pubkey>,
    /// Resolved writable account keys.
    dynamic_rw: Vec<Pubkey>,
    /// Resolved readonly account keys.
    dynamic_ro: Vec<Pubkey>,
}

/// Errors that can occur when parsing an account key.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum AccountKeyError {
    /// An error occurred while converting the account key index to a usize.
    #[error("Error converting index to usize")]
    IndexConvert(#[from] std::num::TryFromIntError),
    /// The account key index was out of range.
    #[error("Invalid account key index {0}")]
    InvalidIndex(usize),
    /// The referenced account key was invalid.
    #[error("Invalid account key data")]
    InvalidKey(#[from] std::array::TryFromSliceError),
}

impl AccountKeys {
    pub fn new(static_keys: &[Pubkey], dynamic_keys: Option<&LoadedAddresses>) -> Self {
        let (dynamic_rw, dynamic_ro) = if let Some(dynamic) = dynamic_keys {
            (dynamic.writable.to_vec(), dynamic.readonly.to_vec())
        } else {
            (vec![], vec![])
        };

        Self {
            static_keys: static_keys.to_vec(),
            dynamic_ro,
            dynamic_rw,
        }
    }

    pub fn new_arc(static_keys: &[Pubkey], dynamic_keys: Option<&LoadedAddresses>) -> Arc<Self> {
        Arc::new(Self::new(static_keys, dynamic_keys))
    }

    /// Get an Account pubkey by index within the Transaction.
    ///
    /// # Errors
    /// Returns an error if the index is invalid.
    pub fn get<I: TryInto<usize>>(&self, idx: I) -> Option<&Pubkey>
    where
        I::Error: Into<std::num::TryFromIntError>,
    {
        let idx = idx.try_into().ok()?;
        let mut i = idx;
        [&self.static_keys, &self.dynamic_rw, &self.dynamic_ro]
            .into_iter()
            .find_map(|k| {
                k.get(i).map_or_else(
                    || {
                        i = i.saturating_sub(k.len());
                        None
                    },
                    Some,
                )
            })
    }

    /// Returns an iterator of account key segments. The ordering of segments
    /// affects how account indexes from compiled instructions are resolved and
    /// so should not be changed.
    #[inline]
    fn key_segment_iter(&self) -> impl Iterator<Item = &[Pubkey]> + Clone {
        [
            self.static_keys.as_slice(),
            self.dynamic_rw.as_slice(),
            self.dynamic_ro.as_slice(),
        ]
        .into_iter()
    }

    /// Returns the total length of loaded accounts for a message
    #[inline]
    pub fn len(&self) -> usize {
        let mut len = 0usize;
        for key_segment in self.key_segment_iter() {
            len = len.saturating_add(key_segment.len());
        }
        len
    }

    /// Returns true if this collection of account keys is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterator for the addresses of the loaded accounts for a message
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Pubkey> + Clone {
        self.key_segment_iter().flatten()
    }
}
