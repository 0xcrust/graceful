pub mod jupiter;

use crate::swap::{Swap, SwapProgram};

use solana_pubkey::Pubkey;

pub struct AggregatorSwap {
    pub program: SwapProgram,
    pub user: Pubkey,
    pub swap: Option<Swap>,
    pub swaps: Option<Vec<Swap>>,
}

pub fn is_aggregator(program: &SwapProgram) -> bool {
    matches!(
        program,
        SwapProgram::AxiomTrade
            | SwapProgram::DflowAgg
            | SwapProgram::JupV6
            | SwapProgram::OKXDexRouter2
            | SwapProgram::TitanExchangeRouter
    )
}
