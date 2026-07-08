//! A parser for Solana transactions that recognizes and decodes swap
//! activity from aggregator/router programs and dex/AMM programs.
//!
//! The main entry point is [`parse::parse_transaction`], which takes a raw
//! transaction, resolves its instructions against a registry of known
//! programs, and returns a list of decoded [`parse::Parsed`] swaps. See the
//! [`parse`] module documentation for details on how instructions are
//! walked, resolved, and parsed, and for the full set of errors that can
//! occur along the way.
//!
//! # Modules
//! - [`parse`]: entry point and dispatch logic for turning a transaction
//!   into recognized swaps. See [`parse::aggregator`] and [`parse::dex`]
//!   for the aggregator- and dex-specific parsing logic respectively.
//! - [`swap`]: shared swap types and the [`swap::DISALLOWED`] program
//!   exclusion list.
//! - [`transaction`]: the [`transaction::SolanaTx`] representation that raw
//!   transactions are converted into before parsing.
//! - [`util`]: shared helpers for working with account keys, balances, and
//!   logs.

pub mod parse;
pub mod swap;
pub mod transaction;
pub mod util;
