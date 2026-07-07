use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::{Program, Swap},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, balances::Addr, logs::TxLogsError},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

const SWAP_BASE_IN: u8 = 9;
const SWAP_BASE_OUT: u8 = 11;
const SWAP_BASE_IN_V2: u8 = 16;
const SWAP_BASE_OUT_V2: u8 = 17;

const V1_ACCOUNTS_LENGTH: usize = 17;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let data = view.ix.data()?;
    let Some(discriminator) = data.first() else {
        return Ok(None);
    };

    let accs =
        InstructionAccounts::new(view.keys.clone(), view.ix.accounts(), view.ix.program_id())?;

    let (coin_vault, pc_vault, market, user) = match *discriminator {
        SWAP_BASE_IN | SWAP_BASE_OUT => {
            // optional `amm_target_orders` makes it somewhat hacky to get coin and pc vault accounts.
            let include_target_orders = accs.len() > V1_ACCOUNTS_LENGTH;
            if include_target_orders {
                (accs[5], accs[6], accs[1], accs[17])
            } else {
                (accs[4], accs[5], accs[1], accs[16])
            }
        }
        SWAP_BASE_IN_V2 | SWAP_BASE_OUT_V2 => (accs[3], accs[4], accs[1], accs[7]),
        _ => return Ok(None),
    };

    let coin_mint = view
        .balance
        .find_token_account_balance(Addr::Key(coin_vault))
        .mint();
    let pc_mint = view
        .balance
        .find_token_account_balance(Addr::Key(pc_vault))
        .mint();

    let (coin_mint, pc_mint) = match (coin_mint, pc_mint) {
        (Some(c), Some(pc)) => (c, pc),
        _ => return Err(ParseError::NoDetailsFound),
    };

    let base_in = match *discriminator {
        SWAP_BASE_IN | SWAP_BASE_IN_V2 => true,
        SWAP_BASE_OUT | SWAP_BASE_OUT_V2 => false,
        _ => unreachable!(),
    };

    let event = match parse_log(&view, base_in) {
        Ok(Some(event)) => Some(event),
        Ok(None) | Err(_) => None,
    };

    let swap = match event {
        Some(event) => {
            let (input_mint, output_mint) = match event.direction {
                SwapDirection::Coin2PC => (coin_mint, pc_mint),
                SwapDirection::PC2Coin => (pc_mint, coin_mint),
            };
            Swap {
                input_mint,
                input_amount: event.amount_in,
                output_mint,
                output_amount: event.amount_out,
            }
        }
        None => {
            let Some(info) = get_swap_info(
                &view.balance,
                &view.keys,
                view.ix,
                coin_vault,
                pc_vault,
                user,
                true,
            )?
            else {
                return Err(ParseError::NoDetailsFound);
            };

            info.into()
        }
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
    base_in: bool,
) -> Result<Option<Event>, TxLogsError> {
    Ok(view.logs.program_logs(view.ix.path())?.find_map(|log| {
        let log = log
            .strip_prefix("Program log: ray_log:")
            .map(|s| s.trim())?;
        let event = decode_log(log).ok()?;
        if event.base_in == base_in {
            Some(event)
        } else {
            None
        }
    }))
}

fn decode_log(log: impl AsRef<str>) -> anyhow::Result<Event> {
    let bytes = STANDARD.decode(log.as_ref())?;
    Ok(match LogType::from_u8(bytes[0])? {
        LogType::SwapBaseIn => {
            let decoded = bincode::deserialize::<SwapBaseInLog>(&bytes)?;
            Event {
                amount_in: decoded.amount_in,
                amount_out: decoded.out_amount,
                direction: SwapDirection::from_u64(decoded.direction)?,
                base_in: true,
            }
        }
        LogType::SwapBaseOut => {
            let decoded = bincode::deserialize::<SwapBaseOutLog>(&bytes)?;
            Event {
                amount_in: decoded.deduct_in,
                amount_out: decoded.amount_out,
                direction: SwapDirection::from_u64(decoded.direction)?,
                base_in: false,
            }
        }
    })
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SwapBaseInLog {
    log_type: u8,
    // input
    amount_in: u64,
    minimum_out: u64,
    direction: u64,
    // user info
    user_source: u64,
    // pool info
    pool_coin: u64,
    pool_pc: u64,
    // calc result
    out_amount: u64,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SwapBaseOutLog {
    log_type: u8,
    // input
    max_in: u64,
    amount_out: u64,
    direction: u64,
    // user info
    user_source: u64,
    // pool info
    pool_coin: u64,
    pool_pc: u64,
    // calc result
    deduct_in: u64,
}

struct Event {
    amount_in: u64,
    amount_out: u64,
    base_in: bool,
    direction: SwapDirection,
}

enum LogType {
    SwapBaseIn,
    SwapBaseOut,
}

impl LogType {
    pub fn from_u8(log_type: u8) -> anyhow::Result<Self> {
        Ok(match log_type {
            3 => LogType::SwapBaseIn,
            4 => LogType::SwapBaseOut,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid log_type for Raydium: {}",
                    log_type
                ));
            }
        })
    }
}

enum SwapDirection {
    /// Input token pc, output token coin
    PC2Coin,
    /// Input token coin, output token pc
    Coin2PC,
}

impl SwapDirection {
    pub fn from_u64(direction: u64) -> anyhow::Result<Self> {
        Ok(match direction {
            1 => SwapDirection::PC2Coin,
            2 => SwapDirection::Coin2PC,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid direction for Raydium: {}",
                    direction
                ));
            }
        })
    }
}
