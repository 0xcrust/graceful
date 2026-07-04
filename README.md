# Solana Swap Parser

> **Status: WIP.** Interfaces, output format, and internals are all still
> subject to change. Nothing here is stable yet — treat this README as a
> placeholder for the real one.

## What this is

A parser that reconstructs a detailed, human-readable summary of a swap's
route on Solana by walking its instructions (including nested CPI calls) and
figuring out what actually happened on-chain — which pools were touched, in
what order, and how the input amount flowed through to the final output.

The goal is to go from "a pile of instructions and token balance changes" to
something like:

```
1.5 SOL -> 42,000 BONK
  via Raydium (SOL -> USDC) -> Orca (USDC -> BONK)
```

including the harder cases: split routes, multi-hop paths, and legs that
cycle back through a mint the route already passed through.

## How it works (so far)

- Elementary swaps extracted from parsed instructions are modeled as directed
  edges between mints (see `swap_graph.rs`: `Swap` / `SwapGraph`).
- `SwapGraph::swap()` collapses a full route into a single net swap.
- `SwapGraph::split()` breaks a route into aggregate legs around a given
  intermediate mint, which is what will eventually drive the step-by-step
  route summary.

## Not done yet

- Instruction-level parsing per DEX program
- CPI traversal / instruction decoding.
- Actual "summary" output format — text, JSON, whatever's most useful.
- Handling of failed/partial swaps.

## Running tests

```
cargo test
```

---
*This README will get a real rewrite once the parsing side lands.*