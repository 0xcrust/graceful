//! Balance-diff-first Solana swap detection.
//!
//! Core principle: a swap is defined by *what changed* (net token/SOL balance
//! deltas per owner) not by *what was called* (instruction data). Instruction
//! data is only used afterward, to label/enrich a swap that was already
//! detected from state.

use solana_client::rpc_response::OptionSerializer;
use solana_pubkey::Pubkey;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionStatusMeta, UiTransactionTokenBalance,
};
use std::collections::HashMap;
use std::str::FromStr;

use crate::transaction::convert_ui_loaded_addresses;
use crate::util::accounts::AccountKeys;

pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// Net change of a single (owner, mint) pair over the whole transaction.
#[derive(Debug, Clone)]
pub struct BalanceDelta {
    pub owner: Pubkey,
    pub mint: String,
    pub delta: i128,
    pub decimals: u8,
}

/// A detected swap: one trader, one or more mints sold, one or more mints bought.
#[derive(Debug, Clone)]
pub struct SwapEvent {
    /// The trader's public key.
    pub trader: Pubkey,
    /// sold: (`publicKey`, `amount`, `mintDecimals`).
    pub sold: Vec<(String, i128, u8)>,
    /// bought: (`publicKey`, `amount`, `mintDecimals`).
    pub bought: Vec<(String, i128, u8)>,
    /// Owner was off-curve: a PDA/escrow account.
    pub is_program_mediated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum TxParseError {
    #[error("Missing transaction meta")]
    MissingMeta,
    #[error("Cannot parse swaps for failed transaction")]
    TransactionFailed,
    #[error("Invalid public key")]
    InvalidPubkey,
    #[error("Failed to decode transaction")]
    FailedDecodeTx,
}

/// Detect swaps for a transaction using a balance-diff algorithm and
/// no instruction parsing.
pub fn detect_swaps(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    fee_payer: Pubkey,
    fee_lamports: u64,
) -> Result<Vec<SwapEvent>, TxParseError> {
    let mut deltas = compute_raw_deltas(tx)?;
    merge_sol_and_wsol(&mut deltas, fee_payer, fee_lamports);
    let swaps = cluster_into_swaps(&deltas);
    Ok(swaps)
}

/// Normalize the transaction and compute raw per-account deltas.
///
/// Returns a map of "token account owner" -> "mint" -> net delta, plus the
/// native SOL deltas keyed by account pubkey.
///
/// ## Note:
/// Fee payer's delta will still include the tx fee at this stage; callers
/// should subtract it (see `merge_sol_and_wsol`).
fn compute_raw_deltas(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<HashMap<(Pubkey, String), BalanceDelta>, TxParseError> {
    let meta = tx
        .transaction
        .meta
        .as_ref()
        .ok_or(TxParseError::MissingMeta)?;

    let tx = tx
        .transaction
        .transaction
        .decode()
        .ok_or(TxParseError::FailedDecodeTx)?;

    if meta.err.is_some() {
        return Err(TxParseError::TransactionFailed);
    }

    let mut deltas: HashMap<(Pubkey, String), BalanceDelta> = HashMap::new();

    let pre_token: &[UiTransactionTokenBalance] = meta
        .pre_token_balances
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let post_token: &[UiTransactionTokenBalance] = meta
        .post_token_balances
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    let mut pre_map: HashMap<u8, &UiTransactionTokenBalance> = HashMap::new();
    for b in pre_token {
        pre_map.insert(b.account_index, b);
    }
    let mut post_map: HashMap<u8, &UiTransactionTokenBalance> = HashMap::new();
    for b in post_token {
        post_map.insert(b.account_index, b);
    }

    // Union of every account index that appears in either snapshot.
    let mut indices: Vec<u8> = pre_map.keys().chain(post_map.keys()).cloned().collect();
    indices.sort_unstable();
    indices.dedup();

    for idx in indices {
        let pre = pre_map.get(&idx);
        let post = post_map.get(&idx);

        // Owner + mint should be identical across pre/post for the same account
        // index; prefer post (it's the more "current" snapshot), fall back to pre.
        //
        // (covers accounts that were closed by the end of the tx).
        let (owner_str, mint, decimals) = match (pre, post) {
            (_, Some(p)) => (
                match p.owner.clone() {
                    OptionSerializer::Some(s) => Some(s),
                    _ => None,
                },
                p.mint.clone(),
                p.ui_token_amount.decimals,
            ),
            (Some(p), None) => (
                match p.owner.clone() {
                    OptionSerializer::Some(s) => Some(s),
                    _ => None,
                },
                p.mint.clone(),
                p.ui_token_amount.decimals,
            ),
            (None, None) => continue,
        };

        let Some(owner_str) = owner_str else {
            continue;
        };
        let owner = Pubkey::from_str(&owner_str).map_err(|_| TxParseError::InvalidPubkey)?;

        let pre_amount: i128 = pre
            .and_then(|p| p.ui_token_amount.amount.parse::<i128>().ok())
            .unwrap_or(0);
        let post_amount: i128 = post
            .and_then(|p| p.ui_token_amount.amount.parse::<i128>().ok())
            .unwrap_or(0);

        let delta_amount = post_amount - pre_amount;
        if delta_amount == 0 {
            continue;
        }

        deltas
            .entry((owner, mint.clone()))
            .and_modify(|d| d.delta += delta_amount)
            .or_insert(BalanceDelta {
                owner,
                mint,
                delta: delta_amount,
                decimals,
            });
    }

    if let (Some(pre_bal), Some(post_bal), Some(account_keys)) = (
        Some(&meta.pre_balances),
        Some(&meta.post_balances),
        get_account_keys(&tx, meta),
    ) {
        for (i, owner) in account_keys.iter().enumerate() {
            let pre = *pre_bal.get(i).unwrap_or(&0) as i128;
            let post = *post_bal.get(i).unwrap_or(&0) as i128;
            let delta_lamports = post - pre;
            if delta_lamports == 0 {
                continue;
            }
            deltas
                .entry((*owner, SOL_MINT.to_string()))
                .and_modify(|d| d.delta += delta_lamports)
                .or_insert(BalanceDelta {
                    owner: *owner,
                    mint: SOL_MINT.to_string(),
                    delta: delta_lamports,
                    decimals: 9,
                });
        }
    }

    Ok(deltas)
}

fn get_account_keys(
    tx: &VersionedTransaction,
    meta: &UiTransactionStatusMeta,
) -> Option<AccountKeys> {
    Some(AccountKeys::new(
        tx.message.static_account_keys(),
        convert_ui_loaded_addresses(meta.loaded_addresses.as_ref())
            .ok()?
            .as_ref(),
    ))
}

/// Merge native SOL and wrapped SOL deltas into one economic asset
/// per real owner, and subtract the tx fee from the fee payer.
fn merge_sol_and_wsol(
    deltas: &mut HashMap<(Pubkey, String), BalanceDelta>,
    fee_payer: Pubkey,
    fee_lamports: u64,
) {
    // Subtract the fee from the fee payer's native SOL delta so it doesn't
    // get misread as part of a swap amount.
    if let Some(d) = deltas.get_mut(&(fee_payer, SOL_MINT.to_string())) {
        d.delta += fee_lamports as i128; // fee already reduced post_balance, so add it back
    }

    // Fold WSOL deltas into the SOL_MINT bucket for the same owner.
    let wsol_keys: Vec<(Pubkey, String)> = deltas
        .keys()
        .filter(|(_, mint)| mint == SOL_MINT)
        .cloned()
        .collect();

    for key in wsol_keys {
        if let Some(wsol_delta) = deltas.remove(&key) {
            let (owner, _) = key;
            deltas
                .entry((owner, SOL_MINT.to_string()))
                .and_modify(|d| d.delta += wsol_delta.delta)
                .or_insert(BalanceDelta {
                    owner,
                    mint: SOL_MINT.to_string(),
                    delta: wsol_delta.delta,
                    decimals: 9,
                });
        }
    }
}

/// Identity resolution and clustering.
///
/// Identity comes from *ownership* (already baked into the delta map's key).
pub fn cluster_into_swaps(deltas: &HashMap<(Pubkey, String), BalanceDelta>) -> Vec<SwapEvent> {
    let mut by_owner: HashMap<Pubkey, Vec<&BalanceDelta>> = HashMap::new();
    for d in deltas.values() {
        by_owner.entry(d.owner).or_default().push(d);
    }

    let mut swaps = Vec::new();

    for (owner, owner_deltas) in by_owner {
        let sold: Vec<(String, i128, u8)> = owner_deltas
            .iter()
            .filter(|d| d.delta < 0)
            .map(|d| (d.mint.clone(), -d.delta, d.decimals))
            .collect();
        let bought: Vec<(String, i128, u8)> = owner_deltas
            .iter()
            .filter(|d| d.delta > 0)
            .map(|d| (d.mint.clone(), d.delta, d.decimals))
            .collect();

        // A swap requires BOTH a negative and a positive leg for the same
        // owner. A lone one-sided delta (fee, tip, dust, incidental transfer
        // that happened to occur before or after the real swap legs) never
        // qualifies. This makes the algorithm order-independent by
        // construction, since deltas were already summed over the whole tx.
        if sold.is_empty() || bought.is_empty() {
            continue;
        }

        swaps.push(SwapEvent {
            trader: owner,
            sold,
            bought,
            is_program_mediated: !is_on_curve(&owner),
        });
    }

    swaps
}

/// Provides an heuristic to tell a genuine user account from a
/// program-controlled intermediate account.
///
/// Real wallets are ed25519 keypairs(on-curve). PDAs (pool vaults,
/// escrow accounts, etc) are off-curve by construction.
pub fn is_on_curve(pubkey: &Pubkey) -> bool {
    pubkey.is_on_curve()
}
