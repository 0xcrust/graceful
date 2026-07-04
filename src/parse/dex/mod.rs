use crate::util::keys::KeyError;

pub mod aggregator;

#[derive(Debug, thiserror::Error)]
pub enum DexParseError {
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    AccountKeys(#[from] KeyError),
    #[error("Error deserializing borsh data: {0}")]
    BorshDeserialize(#[from] std::io::Error),
}
