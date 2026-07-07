use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::{Program, Swap},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, balances::Addr, logs::TxLogsError},
};

use anchor_lang::AnchorDeserialize;
use base64::{Engine, engine::general_purpose::STANDARD};
use solana_pubkey::Pubkey;

const SWAP_BASE_IN: &[u8] = &[143, 190, 90, 218, 196, 30, 51, 222];
const SWAP_BASE_OUT: &[u8] = &[55, 217, 98, 86, 163, 74, 180, 173];

const SWAP_EVENT: &[u8] = &[64, 198, 205, 232, 38, 8, 113, 226];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView { ix, keys, .. } = &view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (input_vault, output_vault, market, user) = match discriminator {
        SWAP_BASE_IN | SWAP_BASE_OUT => (accs[6], accs[7], accs[3], accs[0]),
        _ => return Ok(None),
    };

    let swap = parse_log(&view)
        .ok()
        .flatten()
        .and_then(|(base, exts)| match exts {
            Some(exts) => Some(Swap {
                input_mint: exts.input_mint,
                input_amount: base.input_amount,
                output_mint: exts.output_mint,
                output_amount: base.output_amount,
            }),
            None => {
                let input_mint = view
                    .balance
                    .find_token_account_balance(Addr::Key(input_vault))
                    .mint()?;
                let output_mint = view
                    .balance
                    .find_token_account_balance(Addr::Key(output_vault))
                    .mint()?;

                Some(Swap {
                    input_mint,
                    input_amount: base.input_amount,
                    output_amount: base.output_amount,
                    output_mint,
                })
            }
        });

    let swap = match swap {
        Some(swap) => swap,
        None => get_swap_info(
            &view.balance,
            &view.keys,
            view.ix,
            input_vault,
            output_vault,
            user,
            true,
        )?
        .ok_or(ParseError::NoDetailsFound)?
        .into(),
    };

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::RaydiumV4,
        swap,
    }))
}

fn parse_log<T: SolanaInstruction>(
    view: &IxView<T>,
) -> Result<Option<(BaseEvent, Option<V2Extensions>)>, TxLogsError> {
    Ok(view
        .logs
        .program_logs(view.ix.path())?
        .find_map(|log| decode_log(log.strip_prefix("Program data:").map(|s| s.trim())?)))
}

fn decode_log(log: impl AsRef<str>) -> Option<(BaseEvent, Option<V2Extensions>)> {
    let bytes = STANDARD.decode(log.as_ref()).ok()?;

    if bytes.len() < 8 {
        return None;
    }

    match &bytes[..8] {
        SWAP_EVENT => {
            let buf = &mut &bytes[..];
            let base = BaseEvent::deserialize(buf).ok()?;

            Some((base, V2Extensions::deserialize(buf).ok()))
        }
        _ => None,
    }
}

#[derive(AnchorDeserialize)]
#[allow(dead_code)]
struct BaseEvent {
    pool_id: Pubkey,
    input_vault_before: u64,
    output_vault_before: u64,
    input_amount: u64,
    output_amount: u64,
    input_transfer_fee: u64,
    output_transfer_fee: u64,
    base_input: bool,
}

#[derive(AnchorDeserialize)]
#[allow(dead_code)]
struct V2Extensions {
    input_mint: Pubkey,
    output_mint: Pubkey,
    trade_fee: u64,
    creator_fee: u64,
    creator_fee_on_input: bool,
}
