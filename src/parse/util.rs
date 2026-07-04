//! Two-tier swap detection for a single instruction (or instruction +
//! its inner instructions).
//!
//! Tier 1, [`get_swap_info_from_balance_diff`], is the cheap path: it reads
//! the net balance change on two known token accounts and infers direction
//! from sign alone, without parsing any instruction data. It fails closed
//! (`SwapInfoError::FailedHeuristic`) only when both accounts moved in the
//! same direction — which happens when the same account is touched by more
//! than one instruction in the transaction, so the net diff conflates two
//! separate legs and sign alone can no longer disambiguate.
//!
//! Tier 2, [`get_swap_info_from_token_transfers`], is the fallback: it
//! parses actual `Transfer`/`TransferChecked` instructions and identifies
//! the user's leg by signer, resolving exactly the ambiguity tier 1 can't.
//!
//! [`get_swap_info`] wires the two together: try tier 1, escalate to tier 2
//! only on `FailedHeuristic`, and surface every other error immediately.
//!
//! Types assumed to already exist elsewhere in the host crate and
//! intentionally *not* redefined here: `IxView`, `BorrowedIx`,
//! `TokenTransfer`, `AccountKeys`, and the balance-lookup type returned by
//! `ix.balances.get_balance_for_token_account(..)` (with its `.mint()`,
//! `.token_program()`, `.difference()`, and `.pubkey()` accessors).

use std::sync::Arc;

use crate::{
    swap::Swap,
    transaction::instruction::SolanaInstruction,
    util::{
        balances::{Addr, TokenBalance, TxBalance},
        keys::AccountKeys,
        transfer::{TokenParseError, TokenTransfer, parse_multiple_token_transfers},
    },
};

use solana_pubkey::Pubkey;

pub struct ParsedSwap {
    pub base: Swap,
    pub input_token_program: Pubkey,
    pub output_token_program: Pubkey,
    pub a_to_b: bool,
}

impl From<ParsedSwap> for Swap {
    fn from(value: ParsedSwap) -> Self {
        value.base
    }
}

/// Result of successfully identifying a swap's two legs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapInfo {
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_token_program: Pubkey,
    pub output_token_program: Pubkey,
    /// `true` if the input side is `token_account_a`'s mint.
    pub a_to_b: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SwapInfoError {
    /// Both accounts' balances moved in the same direction, so sign alone
    /// can't tell input from output. Carries `(token_a_is_input, token_b_is_input)`
    /// as observed. Callers should fall back to
    /// [`get_swap_info_from_token_transfers`].
    #[error("balance-diff heuristic ambiguous: token_a_input={0}, token_b_input={1}")]
    FailedHeuristic(bool, bool),

    #[error("token account {account} is missing a resolvable mint")]
    MissingMint { account: Pubkey },

    #[error("token account {account} is missing a resolvable token program")]
    MissingTokenProgram { account: Pubkey },

    #[error("Error parsing token instruction: {0}")]
    Parse(#[from] TokenParseError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error>),
}

impl SwapInfoError {
    fn other<E: std::error::Error + 'static>(e: E) -> Self {
        Self::Other(Box::new(e))
    }
}

/// Tries the cheap balance-diff heuristic first; falls back to parsing
/// token-transfer instructions only if that heuristic was genuinely
/// ambiguous. Any other error is surfaced immediately without a fallback
/// attempt.
pub fn get_swap_info(
    balance: &TxBalance,
    account_keys: &Arc<AccountKeys>,
    ix: impl SolanaInstruction,
    token_account_a: Pubkey,
    token_account_b: Pubkey,
    user: Pubkey,
    is_pool_account: bool,
) -> Result<Option<SwapInfo>, SwapInfoError> {
    match get_swap_info_from_balance_diff(
        balance,
        token_account_a,
        token_account_b,
        is_pool_account,
    ) {
        Ok(info) => return Ok(Some(info)),
        Err(SwapInfoError::FailedHeuristic(a, b)) => {
            log::warn!(
                "balance-diff ambiguous (a_input={a}, b_input={b}), falling back to transfer parsing",
            );
        }
        Err(e) => return Err(e),
    }

    let inner_ixs: Vec<_> = ix.inner_ixs().collect();
    match get_swap_info_from_token_ixs(
        balance,
        account_keys,
        &inner_ixs[..],
        token_account_a,
        token_account_b,
        user,
        is_pool_account,
    ) {
        Ok(None) => {
            log::warn!("transfer-based fallback also found no swap");
            Ok(None)
        }
        other => other,
    }
}

/// Infers a swap's direction purely from the sign of each account's net
/// balance change over the transaction. See module docs for when/why this
/// fails and what to do about it.
pub fn get_swap_info_from_balance_diff(
    balance: &TxBalance,
    token_account_a: Pubkey,
    token_account_b: Pubkey,
    is_pool_account: bool,
) -> Result<SwapInfo, SwapInfoError> {
    let balance_a = balance.find_token_account_balance(Addr::Key(token_account_a));
    let balance_b = balance.find_token_account_balance(Addr::Key(token_account_b));

    let leg_a = TokenLeg::from_balance(&balance_a, token_account_a)?;
    let leg_b = TokenLeg::from_balance(&balance_b, token_account_b)?;

    // Pool accounts: funds flowing *in* (positive delta) mark the input
    // side. User accounts: funds flowing *out* (negative delta) mark it.
    let a_is_input = if is_pool_account {
        leg_a.delta > 0
    } else {
        leg_a.delta < 0
    };
    let b_is_input = if is_pool_account {
        leg_b.delta > 0
    } else {
        leg_b.delta < 0
    };

    let (input, output) = match (a_is_input, b_is_input) {
        (true, true) | (false, false) => {
            return Err(SwapInfoError::FailedHeuristic(a_is_input, b_is_input));
        }
        (true, false) => (leg_a, leg_b),
        (false, true) => (leg_b, leg_a),
    };

    Ok(SwapInfo {
        a_to_b: input.mint == leg_a.mint,
        input_mint: input.mint,
        output_mint: output.mint,
        input_amount: input.delta.unsigned_abs(),
        output_amount: output.delta.unsigned_abs(),
        input_token_program: input.token_program,
        output_token_program: output.token_program,
    })
}

/// A single token account's resolved identity + net balance change for one
/// side of a candidate swap. Exists purely to avoid repeating the same
/// four-field extraction (mint / token program / delta / account) for both
/// `token_account_a` and `token_account_b`.
#[derive(Copy, Clone)]
struct TokenLeg {
    mint: Pubkey,
    token_program: Pubkey,
    delta: i64,
}

impl TokenLeg {
    fn from_balance(balance: &TokenBalance, account: Pubkey) -> Result<Self, SwapInfoError> {
        Ok(Self {
            mint: balance
                .mint()
                .ok_or(SwapInfoError::MissingMint { account })?,
            token_program: balance
                .token_program()
                .ok_or(SwapInfoError::MissingTokenProgram { account })?,
            delta: balance.difference().map_err(SwapInfoError::other)?,
        })
    }
}

/// Which field of a [`TokenTransfer`] identifies "the account we're
/// matching against" for a given role. Parameterizing on this is what lets
/// [`get_swap_info_from_token_transfers`] handle the pool-account and
/// user-account cases with one implementation instead of two near-identical
/// branches.
#[derive(Clone, Copy)]
enum Side {
    Source,
    Destination,
}

impl Side {
    fn account_of(self, t: &TokenTransfer) -> Pubkey {
        match self {
            Side::Source => t.source_account,
            Side::Destination => t.destination_account,
        }
    }

    fn opposite(self) -> Side {
        match self {
            Side::Source => Side::Destination,
            Side::Destination => Side::Source,
        }
    }

    /// When `token_account_a`/`b` are the pool's own vaults, the user's leg
    /// is identifiable by its *destination* (funds moving into the pool);
    /// when they're the user's own accounts, it's identifiable by its
    /// *source* (funds moving out of the user's account).
    fn for_user_leg(is_pool_account: bool) -> Side {
        if is_pool_account {
            Side::Destination
        } else {
            Side::Source
        }
    }
}

/// The largest transfer among `transfers` whose `side` account satisfies
/// `matches`. "Largest" filters out incidental/fee-sized transfers that
/// might otherwise coincidentally match the same account.
fn find_leg(
    transfers: &[TokenTransfer],
    side: Side,
    matches: impl Fn(Pubkey) -> bool,
) -> Option<&TokenTransfer> {
    transfers
        .iter()
        .filter(|t| matches(side.account_of(t)))
        .max_by_key(|t| t.amount)
}

/// Parses inner instructions into token transfers, then delegates to
/// [`get_swap_info_from_token_transfers`]. Convenience wrapper for the
/// common case where you have the raw inner instructions rather than
/// already-parsed transfers.
pub fn get_swap_info_from_token_ixs(
    balance: &TxBalance,
    account_keys: &Arc<AccountKeys>,
    ixs: &[&impl SolanaInstruction],
    token_account_a: Pubkey,
    token_account_b: Pubkey,
    user: Pubkey,
    is_pool_account: bool,
) -> Result<Option<SwapInfo>, SwapInfoError> {
    let transfers = parse_multiple_token_transfers(account_keys, ixs)?;
    get_swap_info_from_token_transfers(
        balance,
        transfers,
        token_account_a,
        token_account_b,
        user,
        is_pool_account,
    )
}

/// Identifies a swap's two legs from already-parsed token transfers, using
/// the *signer* of the user's transfer to disambiguate exactly the case
/// where [`get_swap_info_from_balance_diff`] fails.
///
/// `user` must be an ownership-derived address (e.g. the token account's
/// recorded owner), never a signer-derived one — using a signer as the
/// source of `user` would make this check circular and let a delegate
/// authority masquerade as the trader.
pub fn get_swap_info_from_token_transfers(
    balance: &TxBalance,
    transfers: Vec<TokenTransfer>,
    token_account_a: Pubkey,
    token_account_b: Pubkey,
    user: Pubkey,
    is_pool_account: bool,
) -> Result<Option<SwapInfo>, SwapInfoError> {
    if transfers.len() < 2 {
        return Ok(None);
    }

    let user_side = Side::for_user_leg(is_pool_account);
    let pool_side = user_side.opposite();
    let known_accounts = [token_account_a, token_account_b];

    let Some(user_transfer) = find_leg(&transfers, user_side, |acc| known_accounts.contains(&acc))
    else {
        return Ok(None);
    };
    if user_transfer.signer != user {
        return Ok(None);
    }

    let user_account = user_side.account_of(user_transfer);
    let a_to_b = user_account == token_account_a;
    let counterpart_account = if a_to_b {
        token_account_b
    } else {
        token_account_a
    };

    let Some(pool_transfer) = find_leg(&transfers, pool_side, |acc| acc == counterpart_account)
    else {
        return Ok(None);
    };
    let pool_account = pool_side.account_of(pool_transfer);

    let input = resolve_leg(
        balance,
        user_transfer.mint,
        user_account,
        user_transfer.amount,
    )?;
    let output = resolve_leg(
        balance,
        pool_transfer.mint,
        pool_account,
        pool_transfer.amount,
    )?;

    Ok(Some(SwapInfo {
        a_to_b,
        input_mint: input.mint,
        output_mint: output.mint,
        input_amount: input.delta.unsigned_abs(),
        output_amount: output.delta.unsigned_abs(),
        input_token_program: input.token_program,
        output_token_program: output.token_program,
    }))
}

/// Builds a [`TokenLeg`] for a transfer's account, preferring the mint
/// already carried on the `TokenTransfer` (from `TransferChecked`, when
/// available) and falling back to the balance snapshot's mint otherwise —
/// this mirrors the fallback the original implementation did inline for
/// both branches.
fn resolve_leg(
    balance: &TxBalance,
    transfer_mint: Option<Pubkey>,
    account: Pubkey,
    amount: u64,
) -> Result<TokenLeg, SwapInfoError> {
    let balance = balance.find_token_account_balance(Addr::Key(account));

    let mint = transfer_mint
        .or_else(|| balance.mint())
        .ok_or(SwapInfoError::MissingMint { account })?;
    let token_program = balance
        .token_program()
        .ok_or(SwapInfoError::MissingTokenProgram { account })?;

    Ok(TokenLeg {
        mint,
        token_program,
        delta: amount as i64,
    })
}
