#![allow(unexpected_cfgs)]
#![allow(clippy::too_many_arguments)]

pub mod axiom;
pub mod dflow;
pub mod jupiter;
pub mod okx_router;
pub mod titan_router;

use std::collections::HashMap;

use crate::{
    parse::{DexSwap, IxView, ParseError},
    swap::{
        DISALLOWED, Program, Swap,
        graph::{SwapGraph, SwapGraphError},
    },
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
        Program::JupV6 => jupiter::parse(ix),
        Program::OKXDexRouter2 => okx_router::parse(ix),
        Program::TitanExchangeRouter => titan_router::parse(ix),
        _ => Ok(None),
    }
}

impl<'a, T: SolanaInstruction> IxView<'a, T> {
    fn aggregator_routes(&self) -> Result<Routes, ParseError> {
        Ok(Routes(
            self.ix
                .inner_instructions()
                .filter_map(|ix| {
                    let ix = Self::new_cloned(ix, self);
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
                                parse_multiple_token_transfers(&self.keys, ix.inner_instructions())
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
    pub(super) fn swap(&self) -> Result<Swap, RouteError> {
        let swaps = if self.0.iter().any(|route| !route.is_decoded()) {
            to_swaps(&self.0)?
        } else {
            self.0
                .iter()
                .map(|route| match route {
                    Route::Decoded(dex) => dex.swap,
                    _ => unreachable!("checked above"),
                })
                .collect()
        };

        Ok(SwapGraph::new(swaps).swap()?)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Route> {
        self.0.iter()
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

#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    #[error("undecoded route doesn't have exactly two distinct dominant mints")]
    AmbiguousTransfers,
    #[error("could not resolve direction for leg {0} even after forward/backward propagation")]
    UnresolvedLeg(usize),
    #[error("could not resolve mint for at least one token transfer")]
    UnknownMint,
    #[error("could not resolve swap from list: {0}")]
    Resolution(#[from] SwapGraphError),
}

#[derive(Debug, Clone, Copy)]
struct MintAmount {
    mint: Pubkey,
    amount: u64,
}

enum LegShape {
    Known(Swap),
    /// Two dominant mints pulled from transfers; direction not yet known.
    Candidates(MintAmount, MintAmount),
}

fn to_swaps(routes: &[Route]) -> Result<Vec<Swap>, RouteError> {
    let mut shapes: Vec<LegShape> = routes
        .iter()
        .map(|r| match r {
            Route::Decoded(dex_swap) => Ok(LegShape::Known(dex_swap.swap)),
            Route::Undecoded { transfers, .. } => candidates_from_transfers(transfers),
        })
        .collect::<Result<_, _>>()?;

    // Forward pass: use each resolved leg's output to fix the next leg's input.
    let mut current_mint: Option<Pubkey> = None;
    for shape in shapes.iter_mut() {
        match shape {
            LegShape::Known(s) => current_mint = Some(s.output_mint),
            LegShape::Candidates(a, b) => {
                if let Some(held) = current_mint
                    && let Some(resolved) = resolve(*a, *b, held, true)
                {
                    current_mint = Some(resolved.output_mint);
                    *shape = LegShape::Known(resolved);
                    continue;
                }
                current_mint = None; // anchor lost until the next known leg
            }
        }
    }

    // Backward pass: use each resolved leg's input to fix the previous leg's output.
    let mut next_mint: Option<Pubkey> = None;
    for shape in shapes.iter_mut().rev() {
        match shape {
            LegShape::Known(s) => next_mint = Some(s.input_mint),
            LegShape::Candidates(a, b) => {
                if let Some(held) = next_mint
                    && let Some(resolved) = resolve(*a, *b, held, false)
                {
                    next_mint = Some(resolved.input_mint);
                    *shape = LegShape::Known(resolved);
                    continue;
                }
                next_mint = None;
            }
        }
    }

    shapes
        .into_iter()
        .enumerate()
        .map(|(i, shape)| match shape {
            LegShape::Known(s) => Ok(s),
            LegShape::Candidates(..) => Err(RouteError::UnresolvedLeg(i)),
        })
        .collect()
}

/// Given two candidate mints and a mint known to be shared with a
/// neighboring leg, resolves direction. `as_input` = true means `held`
/// is this leg's input (forward pass); false means `held` is this
/// leg's output (backward pass).
fn resolve(a: MintAmount, b: MintAmount, held: Pubkey, as_input: bool) -> Option<Swap> {
    let (matched, other) = if a.mint == held {
        (a, b)
    } else if b.mint == held {
        (b, a)
    } else {
        return None; // held mint isn't part of this leg at all
    };

    Some(if as_input {
        Swap {
            input_mint: matched.mint,
            input_amount: matched.amount,
            output_mint: other.mint,
            output_amount: other.amount,
        }
    } else {
        Swap {
            input_mint: other.mint,
            input_amount: other.amount,
            output_mint: matched.mint,
            output_amount: matched.amount,
        }
    })
}

fn candidates_from_transfers(transfers: &[TokenTransfer]) -> Result<LegShape, RouteError> {
    let mut totals: HashMap<Pubkey, u64> = HashMap::new();
    for t in transfers {
        let mint = t.mint.as_ref().ok_or(RouteError::UnknownMint)?;
        *totals.entry(*mint).or_insert(0) += t.amount;
    }
    let mut sorted: Vec<_> = totals.into_iter().collect();
    sorted.sort_by_key(|(_, amt)| std::cmp::Reverse(*amt));

    match sorted.as_slice() {
        [(m1, a1), (m2, a2), ..] => Ok(LegShape::Candidates(
            MintAmount {
                mint: *m1,
                amount: *a1,
            },
            MintAmount {
                mint: *m2,
                amount: *a2,
            },
        )),
        _ => Err(RouteError::AmbiguousTransfers),
    }
}
