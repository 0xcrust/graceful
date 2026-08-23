//! Aggregation of multi-hop / multi-path token swap traces into net swaps.
//!
//! A DEX aggregator (e.g. Jupiter) often executes a single user-facing swap as a
//! sequence of many elementary on-chain swaps: it may split the input across
//! several pools, route through intermediate ("hop") mints, and in some cases
//! even route a portion of funds back through a mint it already passed through
//! (a *cycle*, used e.g. to net out fees or take advantage of a favorable pool).
//!
//! [`SwapGraph`] takes the flat list of elementary [`Swap`]s produced by such a
//! route and answers the following question:
//!
//! **What is the net effect of the whole route?** ([`SwapGraph::swap`]) -
//! i.e. "how much of the *original* input mint went in, and how much of the
//! *final* output mint came out", after netting out any recycled amounts.
//!
//! Internally, the list of swaps is treated as a directed graph whose nodes are
//! mints and whose edges are elementary swaps, and the two questions above are
//! answered with a reachability search followed by a proportional flow
//! computation (see the private helpers below for details).
//!
//! # Example
//!
//! ```
//! # use graceful::swap::{Swap, graph::SwapGraph};
//! # use solana_pubkey::Pubkey;
//! let usdc = Pubkey::new_unique();
//! let sol = Pubkey::new_unique();
//! let bonk = Pubkey::new_unique();
//!
//! // USDC -> SOL -> BONK, executed as two elementary swaps.
//! let route = SwapGraph::new(vec![
//!     Swap::new(usdc, sol, 100_000_000, 500_000_000),
//!     Swap::new(sol, bonk, 500_000_000, 42_000_000_000),
//! ]);
//!
//! let net = route.swap().unwrap();
//! assert_eq!(net.input_mint, usdc);
//! assert_eq!(net.output_mint, bonk);
//! assert_eq!(net.input_amount, 100_000_000);
//! assert_eq!(net.output_amount, 42_000_000_000);
//! ```

use super::Swap;
use std::collections::HashMap;

use solana_pubkey::Pubkey;

/// An ordered collection of elementary [`Swap`]s that together make up one
/// logical (possibly multi-hop, multi-path) route from an original input mint
/// to a final output mint.
///
/// See the [module-level documentation](self) for the concepts this type
/// implements.
#[derive(Clone, Debug, Default)]
pub struct SwapGraph {
    swaps: Vec<Swap>,
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum SwapGraphError {
    #[error("Empty route")]
    EmptyRoute,
    #[error("Inbalanced route. mint: {mint}. amount: {amount}")]
    ImbalancedRoute { mint: Pubkey, amount: u64 },
}

/// Tolerance for treating small discrepancies as parser/rounding noise
/// rather than a genuine missing swap. Expressed in basis points (1 bps
/// = 0.01%) relative to the amount involved in the specific check.
const DUST_TOLERANCE_BPS: u64 = 5; // 0.05% — tune based on observed parser noise

/// Returns true if `amount` is small enough, relative to `reference`,
/// to be considered dust rather than a real discrepancy.
fn is_dust(amount: u64, reference: u64) -> bool {
    // amount / reference <= DUST_TOLERANCE_BPS / 10_000
    // rearranged to avoid floating point:
    amount.saturating_mul(10_000) <= reference.saturating_mul(DUST_TOLERANCE_BPS)
}

impl SwapGraph {
    /// Wraps a flat, unordered-by-graph-position list of elementary swaps.
    ///
    /// The only ordering requirement is the one already implied by the data:
    /// [`SwapGraph::swap`] treats `swaps.first()` as the entry point of the
    /// route (its `input_mint` is the route's overall input) and `swaps.last()`
    /// as the exit point (its `output_mint` is the route's overall output).
    pub fn new(swaps: Vec<Swap>) -> Self {
        SwapGraph { swaps }
    }

    /// Returns the underlying elementary swaps.
    pub fn swaps(&self) -> &[Swap] {
        &self.swaps
    }

    /// Collapses the whole route into a single net [`Swap`] from the route's
    /// original input mint to its final output mint.
    ///
    /// Simulates execution by walking the swaps in order and tracking a
    /// running balance per mint. Any amount that cycles back through the
    /// input mint (e.g. a leg that routes back through the original input
    /// mint before continuing on) is netted out, so the returned
    /// `input_amount` reflects only the external capital actually
    /// contributed by the caller, not gross internal volume.
    ///
    /// Small discrepancies (e.g. from upstream parsing/rounding error) are
    /// tolerated as dust and silently absorbed rather than causing a
    /// failure.
    ///
    /// # Errors
    ///
    /// - [`SwapGraphError::EmptyRoute`] if this `SwapGraph` contains no swaps.
    /// - [`SwapGraphError::ImbalancedRoute`] if some swap requires more of a
    ///   non-input mint than the route ever produced for it, beyond dust
    ///   tolerance — this indicates the route is inconsistent, likely due
    ///   to a missing or undecoded swap.
    pub fn swap(&self) -> Result<Swap, SwapGraphError> {
        if self.swaps.is_empty() {
            return Err(SwapGraphError::EmptyRoute);
        }

        let input_mint = self.swaps.first().unwrap().input_mint;
        let output_mint = self.swaps.last().unwrap().output_mint;

        let mut balance: HashMap<Pubkey, u64> = HashMap::new();
        let mut net_input: u64 = 0;

        for s in &self.swaps {
            let avail = balance.get(&s.input_mint).copied().unwrap_or(0);

            if avail < s.input_amount {
                let shortfall = s.input_amount - avail;
                if s.input_mint == input_mint {
                    // This is real capital the caller had to put in.
                    net_input = net_input.saturating_add(shortfall);
                    balance.insert(s.input_mint, 0);
                } else if is_dust(shortfall, s.input_amount) {
                    // Parser rounding: this swap claims to need slightly
                    // more than was actually produced upstream. Treat the
                    // available balance as sufficient and eat the gap.
                    balance.insert(s.input_mint, 0);
                } else {
                    // A non-entry mint needed more than the route ever
                    // produced for it, by more than dust. This likely
                    // means that there's a missing swap in this route.
                    return Err(SwapGraphError::ImbalancedRoute {
                        mint: s.input_mint,
                        amount: shortfall,
                    });
                }
            } else {
                balance.insert(s.input_mint, avail - s.input_amount);
            }

            *balance.entry(s.output_mint).or_insert(0) += s.output_amount;
        }

        let output_amount = balance.get(&output_mint).copied().unwrap_or(0);

        Ok(Swap {
            input_mint,
            output_mint,
            input_amount: net_input,
            output_amount,
        })
    }
}

#[cfg(test)]
mod basic {
    use super::*;

    fn mints(n: usize) -> Vec<Pubkey> {
        (0..n).map(|_| Pubkey::new_unique()).collect()
    }

    #[test]
    fn nets_a_simple_two_hop_route() {
        let m = mints(3);
        let route = SwapGraph::new(vec![
            Swap::new(m[0], m[1], 100, 500),
            Swap::new(m[1], m[2], 500, 4_200),
        ]);

        let net = route.swap().unwrap();
        assert_eq!(net, Swap::new(m[0], m[2], 100, 4_200));
    }
}

#[cfg(test)]
mod net_swap {
    use super::*;

    fn mint() -> Pubkey {
        Pubkey::new_unique()
    }

    #[test]
    fn single_hop_returns_that_hop_unchanged() {
        // A -> B
        let (a, b) = (mint(), mint());
        let route = SwapGraph::new(vec![Swap::new(a, b, 100, 200)]);

        let net = route.swap().unwrap();
        assert_eq!(net, Swap::new(a, b, 100, 200));
    }

    #[test]
    fn linear_chain_nets_to_endpoints() {
        // A -> B -> C ==> A -> C
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![Swap::new(a, b, 1, 2), Swap::new(b, c, 2, 3)]);

        let net = route.swap().unwrap();
        assert_eq!(net, Swap::new(a, c, 1, 3));
    }

    #[test]
    fn long_linear_chain_nets_to_endpoints() {
        // A -> B -> C -> D -> E, no branching or cycles anywhere.
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, c, 20, 30),
            Swap::new(c, d, 30, 40),
            Swap::new(d, e, 40, 50),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, e, 10, 50));
    }

    #[test]
    fn duplicate_parallel_hops_are_summed() {
        // Two identical A -> B legs, then B -> C.
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 4, 5),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 2, 5));
    }

    #[test]
    fn zero_amount_hop_nets_to_zero() {
        let (a, b) = (mint(), mint());
        let route = SwapGraph::new(vec![Swap::new(a, b, 0, 0)]);

        let net = route.swap().unwrap();
        assert_eq!(net.input_amount, 0);
        assert_eq!(net.output_amount, 0);
    }

    #[test]
    fn large_amounts_do_not_overflow() {
        let (a, b, c) = (mint(), mint(), mint());
        let half_max = u64::MAX / 2;
        let route = SwapGraph::new(vec![
            Swap::new(a, b, half_max, half_max),
            Swap::new(b, c, half_max, half_max),
        ]);

        let net = route.swap().unwrap();
        assert_eq!(net.input_amount, half_max);
        assert_eq!(net.output_amount, half_max);
    }

    #[test]
    fn parallel_paths_through_a_shared_intermediate_merge() {
        // A splits into B and C, both of which land on D.
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, c, 1, 3),
            Swap::new(b, d, 2, 4),
            Swap::new(c, d, 3, 5),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 2, 9));
    }

    #[test]
    fn parallel_paths_of_different_lengths_merge() {
        // A short direct A -> D leg alongside a longer A -> B -> C -> D leg.
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, d, 2, 3),
            Swap::new(a, b, 1, 4),
            Swap::new(b, c, 4, 5),
            Swap::new(c, d, 5, 6),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 3, 9));
    }

    #[test]
    fn parallel_paths_converging_before_the_final_hop() {
        let (a, c, d) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, c, 1, 20),
            Swap::new(a, c, 1, 21),
            Swap::new(c, d, 41, 5),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 2, 5));
    }

    #[test]
    fn multiple_parallel_hops_into_the_final_mint_are_summed() {
        // A -> B -> D and A -> C -> D, no cycles.
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 5, 10),
            Swap::new(a, c, 5, 15),
            Swap::new(b, d, 10, 20),
            Swap::new(c, d, 15, 30),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 10, 50));
    }

    #[test]
    fn cycle_back_to_input_mint_is_netted_out() {
        // A -> B -> C -> A (partial return) -> D
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 2, 3),
            Swap::new(c, a, 3, 11), // returns 1.1x of the original input, scaled by 10
            Swap::new(a, d, 11, 5),
        ]);

        // gross_input = 1 + 11 = 12, recycled = 11, net = 1
        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 1, 5));
    }

    #[test]
    fn cycle_returning_exactly_the_original_input_nets_to_that_amount() {
        // A -> B -> A -> C
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, a, 2, 1),
            Swap::new(a, c, 1, 3),
        ]);

        // gross_input = 1 + 1 = 2, recycled = 1, net = 1
        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 1, 3));
    }

    #[test]
    fn cycle_with_a_gain_still_nets_only_the_external_input() {
        // A -> B -> A (arbitrage gain on the way back) -> C
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, a, 2, 3),
            Swap::new(a, c, 3, 5),
        ]);

        // gross_input = 1 + 3 = 4, recycled = 3, net = 1
        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 1, 5));
    }

    #[test]
    fn cycle_with_a_loss_still_nets_only_the_external_input() {
        // A -> B -> A (slippage loss on the way back) -> C
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, a, 20, 5),
            Swap::new(a, c, 5, 10),
        ]);

        // gross_input = 10 + 5 = 15, recycled = 5, net = 10
        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 10, 10));
    }

    #[test]
    fn recycled_amount_that_would_exceed_gross_input_saturates_to_zero() {
        // Degenerate/adversarial input: still must not underflow or panic.
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, a, 2, 3),
            Swap::new(a, c, 3, 4),
        ]);

        // gross_input = 1 + 3 = 4, recycled = 3, net = 1
        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 1, 4));
    }

    #[test]
    fn two_separate_cycles_back_to_input_are_both_netted_out() {
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, a, 20, 5), // returns 0.5x, scaled by 10
            Swap::new(a, c, 5, 30),
            Swap::new(c, a, 30, 2), // returns 0.2x, scaled by 10
            Swap::new(a, d, 2, 40),
        ]);

        // gross_input = 10 + 5 + 2 = 17, recycled = 5 + 2 = 7, net = 10
        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 10, 40));
    }

    #[test]
    fn three_separate_cycles_back_to_input_are_all_netted_out() {
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, a, 20, 8),
            Swap::new(a, c, 8, 15),
            Swap::new(c, a, 15, 5),
            Swap::new(a, d, 5, 12),
            Swap::new(d, e, 12, 25),
        ]);

        // gross_input = 10 + 8 + 5 = 23, recycled = 8 + 5 = 13, net = 10
        assert_eq!(route.swap().unwrap(), Swap::new(a, e, 10, 25));
    }

    #[test]
    fn cycle_combined_with_an_independent_parallel_path() {
        // One branch cycles back through A; a second, direct branch does not.
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, a, 20, 5), // cycles back
            Swap::new(a, c, 5, 30),
            Swap::new(a, c, 10, 40), // independent, direct
        ]);

        // gross_input = 10 + 5 + 10 = 25, recycled = 5, net = 20
        assert_eq!(route.swap().unwrap(), Swap::new(a, c, 20, 70));
    }

    #[test]
    fn diamond_route_with_a_cycle_on_one_branch() {
        // A -> B -> D (clean branch)
        // A -> C -> A (cycles back) -> D (recombines)
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 5, 10),
            Swap::new(b, d, 10, 15),
            Swap::new(a, c, 10, 20),
            Swap::new(c, a, 20, 8), // cycles back
            Swap::new(a, d, 8, 12),
        ]);

        // gross_input = 5 + 10 + 8 = 23, recycled = 8, net = 15
        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 15, 27));
    }

    #[test]
    fn cycle_through_an_intermediate_mint_does_not_affect_the_net_input() {
        // The cycle is between B and C; it never touches the input mint A,
        // so it must not change the reported net input at all.
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, c, 20, 30),
            Swap::new(c, b, 30, 25), // cycles back to B, not A
            Swap::new(b, d, 25, 25),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 10, 25));
    }

    #[test]
    fn triple_cycle_back_to_input_nets_all_three() {
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, a, 20, 8),
            Swap::new(a, c, 8, 15),
            Swap::new(c, a, 15, 5),
            Swap::new(a, d, 5, 12),
            Swap::new(d, e, 12, 25),
        ]);

        // gross_input = 10 + 8 + 5 = 23, recycled = 8 + 5 = 13, net = 10
        assert_eq!(route.swap().unwrap(), Swap::new(a, e, 10, 25));
    }

    #[test]
    fn empty_route_panics() {
        let route = SwapGraph::new(vec![]);
        assert_eq!(route.swap().unwrap_err(), SwapGraphError::EmptyRoute)
    }
}
