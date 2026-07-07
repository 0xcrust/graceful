pub mod graph;
pub mod program;

pub use program::*;

use solana_pubkey::Pubkey;

/// The base swap type.
///
/// A single elementary swap: `input_amount` of `input_mint` was exchanged for
/// `output_amount` of `output_mint`.
///
/// This is typically one leg of a route as reported by a swap program or aggregator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Swap {
    /// The input amount.
    pub input_amount: u64,
    /// The output amount.
    pub output_amount: u64,
    /// The input token.
    pub input_mint: Pubkey,
    /// The output token.
    pub output_mint: Pubkey,
}

impl Swap {
    /// Creates a new elementary swap record.
    pub fn new(
        input_mint: Pubkey,
        output_mint: Pubkey,
        input_amount: u64,
        output_amount: u64,
    ) -> Self {
        Self {
            input_mint,
            output_mint,
            input_amount,
            output_amount,
        }
    }

    /// Returns `true` if `mint` is either the input or the output side of this swap.
    pub fn has_mint(&self, mint: &Pubkey) -> bool {
        self.input_mint == *mint || self.output_mint == *mint
    }
}

/// Swap with decimals.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SwapWithDecimals {
    pub base: Swap,
    pub input_decimals: u8,
    pub output_decimals: u8,
}
