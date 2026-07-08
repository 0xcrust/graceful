# Graceful

## What this is

A parser that reconstructs what actually happened on-chain during a swap on
Solana. It walks a transaction's instructions and decodes the ones that
belong to a recognized aggregator (router) or AMM program, figuring out
which programs were touched, in what order, and how tokens flowed from input
to output.

## How it works

The entry point is `parse::parse_transaction`, which works as follows.

1. Converts the input into a `transaction::SolanaTx` via `TryInto`.
2. Builds shared parsing context once. This context is the account keys,
   decoded program logs, and token or SOL balance deltas, and it's reused
   across every instruction.
3. Walks each root level instruction, resolves its program ID, and skips it
   if the program is unknown or explicitly excluded via `swap::DISALLOWED`.
4. Dispatches recognized instructions to either the aggregator or dex
   parser, producing a unified `parse::Parsed` value for each swap found.

Instructions belonging to unrecognized programs are skipped silently. Only
instructions that are recognized but fail to parse produce an error, and
that error carries the instruction's path and, if it could be resolved, its
program. This lets you trace a failure back to exactly where it happened in
the transaction.

### Basic usage

```rust
use my_crate::parse::{parse_transaction, Parsed};

fn handle_tx(raw_tx: EncodedConfirmedTransactionWithStatusMeta) -> anyhow::Result<()> {
    // `raw_tx` just needs to implement `TryInto<SolanaTx>`.
    let swaps = parse_transaction(raw_tx)?;

    for parsed in swaps {
        match parsed {
            Parsed::Dex(dex_swap) => {
                println!(
                    "dex swap: user {} on market {} via {}",
                    dex_swap.user, dex_swap.market, dex_swap.program
                );
                println!("  {:?}", dex_swap.swap);
            }
            Parsed::Aggregator(agg_swap) => {
                println!("aggregator swap: {:?}", agg_swap);
            }
        }
    }

    Ok(())
}
```

### Handling parse errors

`parse_transaction` returns `Result<Vec<Parsed>, ParseTxError>`. There are
two broad failure modes.

- `ParseTxError::Convert` means the input couldn't be converted into a
  `SolanaTx` at all.
- `ParseTxError::Ix` means a recognized instruction failed to parse. This
  wraps a `parse::WithTrace<ParseError>`, which includes the offending
  instruction's path and program if it was resolvable, so you know exactly
  where in the transaction things went wrong.

```rust
use my_crate::parse::{parse_transaction, ParseTxError};

match parse_transaction(raw_tx) {
    Ok(swaps) => { /* ... */ }
    Err(ParseTxError::Convert(e)) => {
        eprintln!("couldn't read transaction: {e}");
    }
    Err(err @ ParseTxError::Ix(_)) => {
        // `WithTrace`'s Display impl includes the program and instruction
        // path, e.g. "No details found for swap instruction. program:
        // Raydium. path: 3.1"
        eprintln!("failed to parse a recognized swap instruction: {err}");
    }
}
```

Note that an `Ok(vec![])` result is normal and expected for transactions
that don't contain any recognized swaps. That's not an error, it just means
nothing matched.

## Running tests

```
cargo test
```