#![allow(unexpected_cfgs)]
#![allow(clippy::too_many_arguments)]

pub mod axiom;
pub mod dflow;
pub mod jupiter;
pub mod okx_router;
pub mod titan_router;

use crate::{
    parse::{DexSwap, IxView, ParseError},
    swap::{DISALLOWED, Program, Swap, graph::SwapGraph},
    transaction::instruction::{Path, SolanaInstruction},
    util::transfer::{TokenTransfer, parse_multiple_token_transfers},
};

use solana_pubkey::Pubkey;

#[derive(Debug, Clone)]
pub struct AggregatorSwap {
    pub program: Program,
    pub user: Pubkey,
    pub swap: Option<Swap>,
    pub routes: Routes,
}

pub fn is_aggregator(program: &Program) -> bool {
    matches!(
        program,
        Program::AxiomTrade
            | Program::DflowAgg
            | Program::JupV6
            | Program::OKXDexRouter2
            | Program::TitanExchangeRouter
    )
}

pub fn parse<T: SolanaInstruction>(
    ix: IxView<T>,
    program: &Program,
) -> Result<Option<AggregatorSwap>, ParseError> {
    match program {
        Program::AxiomTrade => axiom::parse(ix),
        Program::DflowAgg => dflow::parse(ix),
        Program::JupV6 => dflow::parse(ix),
        Program::OKXDexRouter2 => okx_router::parse(ix),
        Program::TitanExchangeRouter => titan_router::parse(ix),
        _ => Ok(None),
    }
}

impl<'a, T: SolanaInstruction> IxView<'a, T> {
    fn aggregator_routes(&self) -> Result<Routes, ParseError> {
        Ok(Routes(
            self.ix
                .inner_ixs()
                .filter_map(|ix| {
                    let ix = Self::clone_new_from(ix, self);
                    let agg_id = self.program_id().ok()?;
                    let id = ix.program_id().ok()?;

                    if id == agg_id || DISALLOWED.contains(&id) {
                        return None;
                    }

                    Some(ix)
                })
                .map(|view| {
                    let program = view.program()?;
                    let ix = view.ix;
                    Ok(match super::dex::parse(view, &program)? {
                        Some(dex) => Route::Decoded(dex),
                        None => {
                            let transfers =
                                parse_multiple_token_transfers(&self.keys, ix.inner_ixs())
                                    .unwrap_or_default();
                            Route::Undecoded {
                                program,
                                path: ix.path().clone(),
                                transfers,
                            }
                        }
                    })
                })
                .collect::<Result<_, ParseError>>()?,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Routes(Vec<Route>);

impl Routes {
    pub(super) fn swap(&self) -> Option<Swap> {
        if self.0.iter().any(|route| !route.is_decoded()) {
            return None;
        }

        let swaps = self
            .0
            .iter()
            .map(|route| match route {
                Route::Decoded(dex) => dex.swap,
                _ => unreachable!("checked above"),
            })
            .collect();

        SwapGraph::new(swaps).swap().ok()
    }
}

#[derive(Debug, Clone)]
pub enum Route {
    Decoded(DexSwap),
    Undecoded {
        program: Program,
        path: Path,
        transfers: Vec<TokenTransfer>,
    },
}

impl Route {
    fn is_decoded(&self) -> bool {
        matches!(self, Route::Decoded(_))
    }
}
