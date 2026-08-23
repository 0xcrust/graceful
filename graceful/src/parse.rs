//! Parses raw Solana transactions into recognized swap events.
//!
//! This is the entry point for turning a transaction into a structured list
//! of [`Parsed`] swaps. It walks the transaction's top-level instructions,
//! resolves each instruction's program ID against the known program
//! registry, and decodes the instruction data, program logs, and balance
//! deltas into a unified representation for instructions belonging to a
//! recognized aggregator, such as a swap router, or a recognized dex/AMM.
//!
//! Instructions belonging to unrecognized programs are silently skipped
//! rather than treated as errors, as are programs explicitly excluded via
//! [`crate::swap::DISALLOWED`]. Only instructions that are recognized but
//! fail to parse produce a [`ParseTxError`]. That error carries the
//! instruction's [`Path`] and, if it could be resolved, its [`Program`], so
//! failures can be traced back to their exact origin in the transaction.
//!
//! # Submodules
//! - [`aggregator`]: parsing for aggregator/router-style swap programs.
//! - [`dex`]: parsing for dex/AMM swap programs.
//! - [`full`]: additional parsing utilities.
//! - [`util`]: shared helpers for this module.

pub mod aggregator;
pub mod dex;
pub mod full;
pub mod util;

use crate::{
    parse::aggregator::AggregatorSwap,
    swap::DISALLOWED,
    transaction::SolanaTx,
    util::{accounts::AccountKeys, balances::TxBalance, logs::TxLogs, transfer::TokenParseError},
};

use std::fmt;

use solana_pubkey::Pubkey;

use util::SwapInfoError;

use crate::{
    swap::{Program, Swap},
    transaction::instruction::{Path, SolanaInstruction},
    util::{accounts::KeyError, balances::BalanceError, logs::TxLogsError},
};

/// Parses every root-level instruction of a transaction into recognized swaps.
///
/// `tx` is converted into a [`SolanaTx`] via `TryInto`. Shared parsing
/// context, meaning account keys, program logs, and balance deltas, is then
/// built once and reused across all instructions.
///
/// Each root instruction is handled as follows. If the instruction's
/// program ID can't be resolved, or the program is in
/// [`crate::swap::DISALLOWED`], the instruction is skipped without being
/// treated as an error. Otherwise it's parsed via [`parse_instruction`].
///
/// # Errors
/// - Returns `ParseTxError::Convert` if `tx` fails to convert into a [`SolanaTx`].
///
/// - Returns `ParseTxError::Ix` if a recognized instruction fails to parse.
pub fn parse_transaction<E: std::error::Error + 'static, T: TryInto<SolanaTx, Error = E>>(
    tx: T,
) -> Result<Vec<Parsed>, ParseTxError> {
    let tx = tx
        .try_into()
        .map_err(|e| ParseTxError::Convert(Box::new(e)))?;

    let meta = tx.meta.clone();
    let keys = tx.account_keys().clone();
    let logs = TxLogs::new(tx.meta.clone());
    let balance = TxBalance::new(meta.clone(), keys.clone());

    tx.root_instructions()
        .filter_map(|ix| {
            let program_id = keys.get(ix.program_id())?;
            if DISALLOWED.contains(program_id) {
                return None;
            }
            let view = IxView::new(&ix, keys.clone(), logs.clone(), balance.clone());
            Some(
                parse_instruction(view)
                    .transpose()?
                    .map_err(ParseTxError::from),
            )
        })
        .collect()
}

/// Parses a single instruction, given its [`IxView`], into a recognized swap
/// if one is found.
///
/// Returns `Ok(None)` if the instruction's program isn't a recognized
/// aggregator or AMM.
pub fn parse_instruction<T: SolanaInstruction>(
    view: IxView<T>,
) -> Result<Option<Parsed>, WithTrace<ParseError>> {
    with_trace(view, parse_instruction_internal)
}

/// Dispatches a single instruction to the aggregator or dex parser based on
/// its program, producing a unified [`Parsed`] result.
///
/// Returns `Ok(None)` if the program is neither a recognized aggregator nor
/// a recognized dex/AMM.
fn parse_instruction_internal<T: SolanaInstruction>(
    view: IxView<T>,
) -> Result<Option<Parsed>, ParseError> {
    let program = view.program()?;

    Ok(if aggregator::is_aggregator(&program) {
        aggregator::parse(view, &program)?.map(Parsed::Aggregator)
    } else if dex::is_amm(&program) {
        dex::parse(view, &program)?.map(Parsed::Dex)
    } else {
        None
    })
}

/// A single instruction paired with the shared context needed to parse it:
/// the transaction's account keys, decoded program logs, and balance deltas.
pub struct IxView<'a, T> {
    ix: &'a T,
    keys: AccountKeys,
    logs: TxLogs,
    balance: TxBalance,
}

impl<'a, T: SolanaInstruction> IxView<'a, T> {
    /// Builds a view over `ix` using the given shared parsing context.
    fn new(ix: &'a T, keys: AccountKeys, logs: TxLogs, balance: TxBalance) -> Self {
        Self {
            ix,
            keys,
            logs,
            balance,
        }
    }

    /// Re-scopes an existing view to a different instruction `ix`, typically
    /// one of `from`'s inner instructions, cloning `from`'s shared context
    /// rather than rebuilding it.
    fn new_cloned<'b>(ix: &'b T, from: &IxView<'a, T>) -> IxView<'b, T> {
        IxView {
            ix,
            keys: from.keys.clone(),
            logs: from.logs.clone(),
            balance: from.balance.clone(),
        }
    }

    /// Resolves the [`Pubkey`] of the program that owns this instruction.
    fn program_id(&self) -> Result<Pubkey, ParseError> {
        self.keys
            .get(self.ix.program_id() as usize)
            .copied()
            .ok_or(ParseError::NoProgramId)
    }

    /// Resolves this instruction's program ID and classifies it as a known
    /// [`Program`].
    fn program(&self) -> Result<Program, ParseError> {
        Ok(Program::from(self.program_id()?))
    }
}

/// A successfully parsed swap.
#[derive(Debug, Clone)]
pub enum Parsed {
    /// A swap routed through an aggregator program.
    Aggregator(AggregatorSwap),
    /// A swap executed directly against a dex/AMM program.
    Dex(DexSwap),
}

/// A swap parsed from an AMM instruction.
#[derive(Debug, Clone)]
pub struct DexSwap {
    /// The account that initiated the swap.
    pub user: Pubkey,
    /// The market/pool the swap was executed against.
    pub market: Pubkey,
    /// The dex/AMM program that executed the swap.
    pub program: Program,
    /// The decoded swap details: tokens and amounts involved.
    pub swap: Swap,
}

/// Errors that can occur while parsing an entire transaction.
#[derive(Debug, thiserror::Error)]
pub enum ParseTxError {
    /// The input failed to convert into a [`SolanaTx`].
    #[error("Error converting transaction: {0}")]
    Convert(Box<dyn std::error::Error>),
    /// A recognized instruction failed to parse.
    #[error("Error parsing instruction: {0}")]
    Ix(#[from] WithTrace<ParseError>),
}

/// Errors that can occur while parsing a single instruction.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Catch-all for errors from lower-level parsing steps.
    #[error("Generic error: {0}")]
    Generic(#[from] Box<dyn std::error::Error>),

    /// Failed to resolve an account key referenced by the instruction.
    #[error("Failed key transversal: {0}")]
    KeyTransversal(#[from] KeyError),

    /// Failed to Borsh-deserialize instruction data.
    #[error("Error deserializing borsh data: {0}")]
    BorshDeserialize(#[from] std::io::Error),

    /// Failed to base64-decode program logs.
    #[error("Error decoding base64 program logs: {0}")]
    ProgramLogsDecode(base64::DecodeError),

    /// Failed to retrieve or parse transaction logs.
    #[error("Failed getting tx logs: {0}")]
    TxLogs(#[from] TxLogsError),

    /// The instruction's program ID account could not be resolved.
    #[error("Failed to get program ID for instruction")]
    NoProgramId,

    /// Failed to compute token or SOL balance deltas for the instruction.
    #[error("Balance error: {0}")]
    Balance(#[from] BalanceError),

    /// Failed to extract swap-relevant info from logs or CPI data.
    #[error("Swap info error: {0}")]
    GetSwapInfo(#[from] SwapInfoError),

    /// The instruction matched a known program, but no usable swap details
    /// could be found.
    #[error("No details found for swap instruction")]
    NoDetailsFound,

    /// Failed to parse a token transfer.
    #[error(transparent)]
    TokenParse(#[from] TokenParseError),

    /// Failed to convert the transaction into the expected format.
    #[error("Failed to convert tx: {0}")]
    TxConvert(Box<dyn std::error::Error>),

    /// Any other error not covered by a more specific variant.
    #[error(transparent)]
    Any(#[from] anyhow::Error),
}

/// Runs `f` over `view`. On failure, wraps the error with the originating
/// instruction's [`Path`] and, if it could be resolved, its [`Program`], so
/// callers can trace a parse failure back to its exact instruction.
fn with_trace<T: SolanaInstruction, E, R>(
    view: IxView<T>,
    f: impl Fn(IxView<T>) -> Result<Option<R>, E>,
) -> Result<Option<R>, WithTrace<E>> {
    let path = view.ix.path().clone();
    let program = view.keys.get(view.ix.program_id()).map(Program::from);
    f(view).map_err(|error| WithTrace {
        path,
        program,
        error,
    })
}

/// Wraps an error with the instruction path it originated from, along with
/// the program if it was known, for easier debugging of parse failures.
#[derive(Debug)]
pub struct WithTrace<E> {
    path: Path,
    program: Option<Program>,
    error: E,
}

impl<E: fmt::Display> fmt::Display for WithTrace<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.program {
            Some(program) => write!(
                f,
                "{}. program: {}. path: {}",
                self.error, program, self.path
            ),
            None => write!(f, "{}. program: unknown. path: {}", self.error, self.path),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for WithTrace<E> {}
