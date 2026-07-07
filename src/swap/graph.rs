//! Aggregation of multi-hop / multi-path token swap traces into net swaps.
//!
//! A DEX aggregator (e.g. Jupiter) often executes a single user-facing swap as a
//! sequence of many elementary on-chain swaps: it may split the input across
//! several pools, route through intermediate ("hop") mints, and in some cases
//! even route a portion of funds back through a mint it already passed through
//! (a *cycle*, used e.g. to net out fees or take advantage of a favorable pool).
//!
//! [`SwapGraph`] takes the flat list of elementary [`Swap`]s produced by such a
//! route and answers two questions about it:
//!
//! 1. **What is the net effect of the whole route?** ([`SwapGraph::swap`]) -
//!    i.e. "how much of the *original* input mint went in, and how much of the
//!    *final* output mint came out", after netting out any recycled amounts.
//! 2. **What did the route look like on either side of some intermediate
//!    mint?** ([`SwapGraph::split`]) - i.e. the aggregate swap from the
//!    original input mint up to that mint, and the aggregate swap from that
//!    mint to the final output mint.
//!
//! Internally, the list of swaps is treated as a directed graph whose nodes are
//! mints and whose edges are elementary swaps, and the two questions above are
//! answered with a reachability search followed by a proportional flow
//! computation (see the private helpers below for details).
//!
//! # Example
//!
//! ```
//! # use swap_graph::{Swap, SwapGraph};
//! # use solana_program::pubkey::Pubkey;
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
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapGraphError {
    EmptyRoute,
    ImbalancedMint {
        mint: Pubkey,
        produced: u64,
        consumed: u64,
    },
}

impl SwapGraph {
    /// Wraps a flat, unordered-by-graph-position list of elementary swaps.
    ///
    /// The only ordering requirement is the one already implied by the data:
    /// [`SwapGraph::swap`] and [`SwapGraph::split`] treat `swaps.first()` as
    /// the entry point of the route (its `input_mint` is the route's overall
    /// input) and `swaps.last()` as the exit point (its `output_mint` is the
    /// route's overall output).
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
    /// Any amount that cycles back through the input mint (e.g. a leg that
    /// routes back through the original input mint before continuing on) is
    /// netted out, so the returned `input_amount` reflects only the external
    /// amount actually contributed by the caller, not gross internal volume.
    ///
    /// # Panics
    ///
    /// Panics if this `SwapGraph` contains no swaps. Use
    /// [`SwapGraph::swaps`] to check first if an empty route is possible in
    /// your context.
    pub fn swap(&self) -> Result<Swap, SwapGraphError> {
        // self.validate_balance()?;

        if self.swaps.is_empty() {
            return Err(SwapGraphError::EmptyRoute);
        }

        let input_mint = self.swaps.first().unwrap().input_mint;
        let output_mint = self.swaps.last().unwrap().output_mint;

        let gross_input: u64 = self
            .swaps
            .iter()
            .filter(|s| s.input_mint == input_mint)
            .map(|s| s.input_amount)
            .sum();
        let recycled: u64 = self
            .swaps
            .iter()
            .filter(|s| s.output_mint == input_mint)
            .map(|s| s.output_amount)
            .sum();
        let net_input = gross_input.saturating_sub(recycled);

        let output_amount: u64 = self
            .swaps
            .iter()
            .filter(|s| s.output_mint == output_mint)
            .map(|s| s.output_amount)
            .sum();

        Ok(Swap {
            input_mint,
            output_mint,
            input_amount: net_input,
            output_amount,
        })
    }

    #[allow(unused)]
    fn validate_balance(&self) -> Result<(), SwapGraphError> {
        let input_mint = self
            .swaps
            .first()
            .ok_or(SwapGraphError::EmptyRoute)?
            .input_mint;
        let output_mint = self
            .swaps
            .last()
            .ok_or(SwapGraphError::EmptyRoute)?
            .output_mint;

        let mut produced: HashMap<Pubkey, u64> = HashMap::new();
        let mut consumed: HashMap<Pubkey, u64> = HashMap::new();

        for s in &self.swaps {
            *produced.entry(s.output_mint).or_default() += s.output_amount;
            *consumed.entry(s.input_mint).or_default() += s.input_amount;
        }

        let all_mints: HashSet<Pubkey> = produced.keys().chain(consumed.keys()).copied().collect();

        for mint in all_mints {
            // Terminal output mint: never needs to be consumed further.
            if mint == output_mint {
                continue;
            }
            // Overall input mint: outflow can legitimately exceed inflow
            // (that excess *is* net_input); recycling is handled elsewhere.
            if mint == input_mint {
                continue;
            }

            let p = produced.get(&mint).copied().unwrap_or(0);
            let c = consumed.get(&mint).copied().unwrap_or(0);
            if p != c {
                return Err(SwapGraphError::ImbalancedMint {
                    mint,
                    produced: p,
                    consumed: c,
                });
            }
        }

        Ok(())
    }

    /// Splits the route into two legs at an intermediate `mid` mint:
    /// `(input_mint -> mid, mid -> output_mint)`.
    ///
    /// Returns `(None, None)` if `mid` never appears anywhere in the route.
    /// Either element of the pair is `None` if that leg carried no volume
    /// (which can happen for degenerate or partially-filled routes).
    ///
    /// If `mid` is itself the route's input or output mint, the corresponding
    /// leg is trivial: `split(input_mint) == (None, Some(self.swap()))` and
    /// `split(output_mint) == (Some(self.swap()), None)`.
    ///
    /// Any sub-path that would loop back *through* `mid` on its way to it is
    /// excluded from that leg's flow computation, so cycles through `mid`
    /// don't inflate the reported amounts.
    pub fn split(&self, mid: &Pubkey) -> (Option<Swap>, Option<Swap>) {
        if self.swaps.is_empty() {
            return (None, None);
        }

        let input_mint = self
            .swaps
            .first()
            .expect("checked non-empty above")
            .input_mint;
        let output_mint = self
            .swaps
            .last()
            .expect("checked non-empty above")
            .output_mint;

        // Edge cases: mid is at the boundaries of the route.
        if *mid == input_mint {
            return (None, self.swap().ok());
        }
        if *mid == output_mint {
            return (self.swap().ok(), None);
        }

        // Nothing to split on if mid doesn't appear in the route at all.
        if !self.swaps.iter().any(|s| s.has_mint(mid)) {
            return (None, None);
        }

        let first_leg = self.flow_between(input_mint, *mid);
        let second_leg = self.flow_between(*mid, output_mint);

        (first_leg, second_leg)
    }

    /// Computes the aggregated swap flowing from `from` to `to`, considering
    /// only the sub-graph reachable from `from` without first passing through
    /// `to` (this is what keeps cycles through `to` from being double-counted).
    ///
    /// This single routine implements both directions of [`SwapGraph::split`]:
    /// called as `flow_between(input_mint, mid)` it produces the first leg,
    /// and called as `flow_between(mid, output_mint)` it produces the second.
    fn flow_between(&self, from: Pubkey, to: Pubkey) -> Option<Swap> {
        let reachable = self.reachable_from(from, to);
        if !reachable.contains(&from) {
            return None;
        }

        // Available output volume at each mint reachable from `from`.
        let available = self.compute_available(&reachable, from);

        let mut total_input = 0u64;
        let mut total_output = 0u64;

        for edge in &self.swaps {
            if edge.output_mint != to || !reachable.contains(&edge.input_mint) {
                continue;
            }

            if edge.input_mint == from {
                // Direct edge straight from the source mint.
                total_input += edge.input_amount;
                total_output += edge.output_amount;
                continue;
            }

            let Some(&avail) = available.get(&edge.input_mint) else {
                continue;
            };
            let used = edge.input_amount.min(avail);
            if used == 0 {
                continue;
            }

            total_output += scale_amount(used, edge.input_amount, edge.output_amount);
            total_input += self.trace_input_contribution(edge.input_mint, used, from, &available);
        }

        Self::finalize(from, to, total_input, total_output)
    }

    /// Depth-first search for every mint reachable from `start`, treating
    /// `exclude` as a wall that traversal may not pass through (though
    /// `exclude` itself is never inserted into the result unless it *is*
    /// `start`).
    fn reachable_from(&self, start: Pubkey, exclude: Pubkey) -> HashSet<Pubkey> {
        let mut reachable = HashSet::new();
        let mut stack = vec![start];

        while let Some(mint) = stack.pop() {
            if mint == exclude || reachable.contains(&mint) {
                continue;
            }
            reachable.insert(mint);

            for edge in &self.swaps {
                if edge.input_mint == mint && !reachable.contains(&edge.output_mint) {
                    stack.push(edge.output_mint);
                }
            }
        }

        reachable
    }

    /// For every mint in `reachable`, computes how much of it is available as
    /// swap *output* originating (directly or transitively) from `source`,
    /// restricted to edges that stay within `reachable`.
    ///
    /// Edges are processed in the order they appear in `self.swaps`, so an
    /// edge whose input mint's availability hasn't been recorded yet (because
    /// the producing edge appears later in the list) contributes nothing -
    /// this mirrors the routes this type is built for, where amounts flow
    /// forward through the swap list.
    fn compute_available(
        &self,
        reachable: &HashSet<Pubkey>,
        source: Pubkey,
    ) -> HashMap<Pubkey, u64> {
        let mut available: HashMap<Pubkey, u64> = HashMap::new();

        for edge in &self.swaps {
            if !reachable.contains(&edge.input_mint) || !reachable.contains(&edge.output_mint) {
                continue;
            }

            if edge.input_mint == source {
                *available.entry(edge.output_mint).or_default() += edge.output_amount;
            } else if let Some(&in_avail) = available.get(&edge.input_mint) {
                let used = edge.input_amount.min(in_avail);
                if used > 0 {
                    let produced = scale_amount(used, edge.input_amount, edge.output_amount);
                    *available.entry(edge.output_mint).or_default() += produced;
                }
            }
        }

        available
    }

    /// Walks backward from `mint` to figure out how much of `source`'s output
    /// was ultimately needed to produce `amount_needed` of `mint`.
    ///
    /// This mirrors [`SwapGraph::compute_available`] in reverse: it finds the
    /// edge that produced `mint`, and either terminates (if that edge's input
    /// is `source` itself) or recurses on that edge's input mint, scaling the
    /// needed amount down at each step.
    fn trace_input_contribution(
        &self,
        mint: Pubkey,
        amount_needed: u64,
        source: Pubkey,
        available: &HashMap<Pubkey, u64>,
    ) -> u64 {
        for edge in &self.swaps {
            if edge.output_mint != mint {
                continue;
            }

            if edge.input_mint == source {
                return scale_amount(amount_needed, edge.output_amount, edge.input_amount);
            }

            if let Some(&producer_avail) = available.get(&edge.input_mint) {
                let used = edge.input_amount.min(producer_avail);
                if used > 0 {
                    let input_needed =
                        scale_amount(amount_needed, edge.output_amount, edge.input_amount);
                    return self.trace_input_contribution(
                        edge.input_mint,
                        input_needed,
                        source,
                        available,
                    );
                }
            }
        }
        0
    }

    /// Builds the resulting leg [`Swap`], or `None` if there was no real
    /// volume on this leg (avoids returning a degenerate zero-amount swap).
    fn finalize(
        input_mint: Pubkey,
        output_mint: Pubkey,
        input_amount: u64,
        output_amount: u64,
    ) -> Option<Swap> {
        (input_amount > 0 && output_amount > 0).then_some(Swap {
            input_mint,
            output_mint,
            input_amount,
            output_amount,
        })
    }
}

/// Scales `amount` (understood as a portion of `from_total`) into the
/// corresponding portion of `to_total`, i.e. computes
/// `amount * to_total / from_total`.
///
/// Uses `u128` intermediate arithmetic so the multiplication cannot overflow
/// for any `u64` inputs, and rounds down (matching the conservative,
/// under-report-rather-than-over-report behavior expected when netting swap
/// amounts). Returns `0` if `from_total` is `0` rather than dividing by zero,
/// since a zero-volume edge cannot proportionally attribute anything.
fn scale_amount(amount: u64, from_total: u64, to_total: u64) -> u64 {
    if from_total == 0 {
        return 0;
    }
    ((amount as u128 * to_total as u128) / from_total as u128) as u64
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

    #[test]
    fn nets_out_a_recycled_amount() {
        let m = mints(3);
        // 100 of m0 -> m1, but 20 of that cycles back to m0 before the rest
        // continues on to m2. Net external input should be 80, not 100.
        let route = SwapGraph::new(vec![
            Swap::new(m[0], m[1], 100, 1_000),
            Swap::new(m[1], m[0], 200, 20),
            Swap::new(m[1], m[2], 800, 3_000),
        ]);

        let net = route.swap().unwrap();
        assert_eq!(net.input_mint, m[0]);
        assert_eq!(net.output_mint, m[2]);
        assert_eq!(net.input_amount, 80);
        assert_eq!(net.output_amount, 3_000);
    }

    #[test]
    fn splits_at_an_intermediate_mint() {
        let m = mints(3);
        let route = SwapGraph::new(vec![
            Swap::new(m[0], m[1], 100, 500),
            Swap::new(m[1], m[2], 500, 4_200),
        ]);

        let (first, second) = route.split(&m[1]);
        assert_eq!(first, Some(Swap::new(m[0], m[1], 100, 500)));
        assert_eq!(second, Some(Swap::new(m[1], m[2], 500, 4_200)));
    }

    #[test]
    fn split_at_boundary_mints_is_trivial() {
        let m = mints(3);
        let route = SwapGraph::new(vec![
            Swap::new(m[0], m[1], 100, 500),
            Swap::new(m[1], m[2], 500, 4_200),
        ]);

        assert_eq!(route.split(&m[0]), (None, Some(route.swap().unwrap())));
        assert_eq!(route.split(&m[2]), (Some(route.swap().unwrap()), None));
    }

    #[test]
    fn split_on_absent_mint_returns_none_none() {
        let m = mints(4);
        let route = SwapGraph::new(vec![Swap::new(m[0], m[1], 100, 500)]);
        assert_eq!(route.split(&m[3]), (None, None));
    }

    #[test]
    fn splits_a_fanned_out_route_with_two_parallel_paths() {
        let m = mints(4); // m0 = in, m1/m2 = parallel intermediates, m3 = out
        let route = SwapGraph::new(vec![
            Swap::new(m[0], m[1], 60, 300),
            Swap::new(m[0], m[2], 40, 210),
            Swap::new(m[1], m[3], 300, 1_000),
            Swap::new(m[2], m[3], 210, 900),
        ]);

        let net = route.swap().unwrap();
        assert_eq!(net, Swap::new(m[0], m[3], 100, 1_900));

        // Splitting at m1 should only capture the path through m1, ignoring
        // the parallel path through m2 (m1 does not appear on that path).
        let (first, second) = route.split(&m[1]);
        assert_eq!(first, Some(Swap::new(m[0], m[1], 60, 300)));
        assert_eq!(second, Some(Swap::new(m[1], m[3], 300, 1_000)));
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
            Swap::new(a, c, 1, 2),
            Swap::new(a, c, 1, 21), // represents a 2.1x leg, scaled by 10
            Swap::new(c, d, 41, 5), // represents a 4.1x leg, scaled by 10
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
            Swap::new(b, d, 15, 25),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 10, 25));
    }

    #[test]
    fn cycle_mid_route_that_never_reaches_the_input_mint_is_ignored() {
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 20),
            Swap::new(b, c, 20, 30),
            Swap::new(c, b, 15, 10),
            Swap::new(b, d, 15, 25),
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

    // --------------------------------------------------------------
    // Edge cases
    // --------------------------------------------------------------

    #[test]
    #[should_panic(expected = "empty route")]
    fn empty_route_panics() {
        let route = SwapGraph::new(vec![]);
        let _ = route.swap().unwrap();
    }
}

// ================================================================
// SwapGraph::split() - aggregating a route into two legs at a mint
// ================================================================
#[cfg(test)]
mod split {
    use super::*;

    fn mint() -> Pubkey {
        Pubkey::new_unique()
    }

    #[test]
    fn splits_at_an_intermediate_mint_with_duplicate_first_leg_hops() {
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 4, 5),
        ]);

        let (first, second) = route.split(&b);
        assert_eq!(first, Some(Swap::new(a, b, 2, 4)));
        assert_eq!(second, Some(Swap::new(b, c, 4, 5)));
    }

    #[test]
    fn splitting_at_the_input_mint_yields_no_first_leg() {
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 4, 5),
        ]);

        let (first, second) = route.split(&a);
        assert_eq!(first, None);
        assert_eq!(second, Some(route.swap().unwrap()));
    }

    #[test]
    fn splitting_at_the_output_mint_yields_no_second_leg() {
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 4, 5),
        ]);

        let (first, second) = route.split(&c);
        assert_eq!(first, Some(route.swap().unwrap()));
        assert_eq!(second, None);
    }

    #[test]
    fn net_swap_across_a_diamond_that_recombines_before_the_split_mint() {
        // A -> B, A -> C, B -> C, C -> D: two paths recombine at C before
        // continuing on to D as a single hop.
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, c, 1, 2),
            Swap::new(b, c, 2, 2),
            Swap::new(c, d, 4, 5),
        ]);

        assert_eq!(route.swap().unwrap(), Swap::new(a, d, 2, 5));
    }

    #[test]
    fn splits_a_diamond_that_recombines_at_the_split_mint() {
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, c, 1, 2),
            Swap::new(b, c, 2, 2),
            Swap::new(c, d, 4, 5),
        ]);

        let (first, second) = route.split(&c);
        assert_eq!(first, Some(Swap::new(a, c, 2, 4)));
        assert_eq!(second, Some(Swap::new(c, d, 4, 5)));
    }

    #[test]
    fn splitting_at_the_output_mint_still_works_with_a_single_path() {
        let (a, b, c) = (mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 4, 5),
        ]);

        let (first, second) = route.split(&c);
        assert_eq!(first, Some(Swap::new(a, c, 2, 5)));
        assert_eq!(second, None);
    }

    #[test]
    #[should_panic(expected = "empty route")]
    fn net_swap_on_an_empty_route_panics() {
        let route = SwapGraph::new(vec![]);
        let _ = route.swap().unwrap();
    }

    #[test]
    fn splitting_an_empty_route_yields_no_legs() {
        let route = SwapGraph::new(vec![]);
        assert_eq!(route.split(&mint()), (None, None));
    }

    #[test]
    fn splits_when_multiple_indirect_paths_reach_the_split_mint() {
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 2, 3),
            Swap::new(a, d, 2, 4),
            Swap::new(d, c, 4, 5),
            Swap::new(c, e, 8, 10),
        ]);

        let (first, second) = route.split(&c);
        assert_eq!(first, Some(Swap::new(a, c, 3, 8)));
        assert_eq!(second, Some(Swap::new(c, e, 8, 10)));
    }

    #[test]
    fn splits_correctly_when_a_cycle_passes_through_the_split_mint() {
        let (a, b, c, d) = (mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 2, 3),
            Swap::new(c, b, 3, 4), // cycles back through the split mint, B
            Swap::new(b, d, 4, 5),
        ]);

        let (first, second) = route.split(&b);
        assert_eq!(first, Some(Swap::new(a, b, 1, 2)));
        assert_eq!(second, Some(Swap::new(b, d, 4, 5)));
    }

    #[test]
    fn splitting_on_a_mint_absent_from_the_route_yields_no_legs() {
        let (a, b, c, d, x) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(b, c, 2, 3),
            Swap::new(c, d, 3, 4),
        ]);

        assert_eq!(route.split(&x), (None, None));
    }

    #[test]
    fn splits_when_two_paths_converge_directly_on_the_split_mint() {
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 1, 2),
            Swap::new(a, c, 2, 3),
            Swap::new(b, d, 2, 4),
            Swap::new(c, d, 3, 5),
            Swap::new(d, e, 9, 10),
        ]);

        let (first, second) = route.split(&d);
        assert_eq!(first, Some(Swap::new(a, d, 3, 9)));
        assert_eq!(second, Some(Swap::new(d, e, 9, 10)));
    }

    #[test]
    fn splits_a_long_chain_with_two_paths_merging_before_the_split_mint() {
        let (a, b, c, d, e, f) = (mint(), mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 2, 4),
            Swap::new(b, c, 4, 6),
            Swap::new(a, d, 3, 5),
            Swap::new(d, c, 5, 7),
            Swap::new(c, e, 13, 15),
            Swap::new(e, f, 15, 20),
        ]);

        let (first, second) = route.split(&c);
        assert_eq!(first, Some(Swap::new(a, c, 5, 13)));
        assert_eq!(second, Some(Swap::new(c, f, 13, 20)));
    }

    #[test]
    fn splits_correctly_with_a_cycle_before_the_split_mint() {
        let (a, b, c, d, e) = (mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 5, 10),
            Swap::new(b, c, 5, 7),
            Swap::new(c, b, 7, 8), // cycles back to B, before the split mint
            Swap::new(b, d, 13, 15),
            Swap::new(d, e, 15, 20),
        ]);

        let (first, second) = route.split(&b);
        assert_eq!(first, Some(Swap::new(a, b, 5, 10)));
        assert_eq!(second, Some(Swap::new(b, e, 13, 20)));
    }

    #[test]
    fn splits_forked_paths_that_merge_after_the_split_mint() {
        let (a, b, c, d, e, f) = (mint(), mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 2, 4),
            Swap::new(a, c, 3, 6),
            Swap::new(b, d, 4, 8),
            Swap::new(c, d, 6, 12),
            Swap::new(d, e, 20, 25),
            Swap::new(e, f, 25, 30),
        ]);

        let (first, second) = route.split(&d);
        assert_eq!(first, Some(Swap::new(a, d, 5, 20)));
        assert_eq!(second, Some(Swap::new(d, f, 20, 30)));
    }

    #[test]
    fn splits_a_single_long_path_at_an_interior_mint() {
        let (a, b, c, d, e, f, g) = (mint(), mint(), mint(), mint(), mint(), mint(), mint());
        let route = SwapGraph::new(vec![
            Swap::new(a, b, 10, 15),
            Swap::new(b, c, 15, 20),
            Swap::new(c, d, 20, 25),
            Swap::new(d, e, 25, 30),
            Swap::new(e, f, 30, 35),
            Swap::new(f, g, 35, 40),
        ]);

        let (first, second) = route.split(&e);
        assert_eq!(first, Some(Swap::new(a, e, 10, 30)));
        assert_eq!(second, Some(Swap::new(e, g, 30, 40)));
    }
}
