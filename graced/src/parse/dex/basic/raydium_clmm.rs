use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::{Program, Swap},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, balances::Addr, logs::TxLogsError},
};

use anchor_lang::AnchorDeserialize;
use base64::{Engine, engine::general_purpose::STANDARD};
use solana_pubkey::Pubkey;

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];

const SWAP_EVENT: &[u8] = &[64, 198, 205, 232, 38, 8, 113, 226];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix,
        ref keys,
        ref balance,
        ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (market, user, input_vault, output_vault) = match discriminator {
        SWAP | SWAP_V2 => (accs[2], accs[0], accs[5], accs[6]),
        _ => return Ok(None),
    };

    let program = Program::RaydiumClmm;

    let log = parse_log(&view)?;

    let swap = match log {
        Some(event) => {
            let input_mint = balance
                .find_token_account_balance(Addr::Key(input_vault))
                .mint();
            let output_mint = balance
                .find_token_account_balance(Addr::Key(output_vault))
                .mint();

            let (input_mint, output_mint) = match (input_mint, output_mint) {
                (Some(input), Some(output)) => (input, output),
                _ => return Err(ParseError::NoDetailsFound),
            };

            let (input_amount, output_amount) = if event.zero_for_one {
                (event.amount0, event.amount1)
            } else {
                (event.amount1, event.amount0)
            };
            Swap {
                input_amount,
                input_mint,
                output_amount,
                output_mint,
            }
        }
        None => match get_swap_info(balance, keys, ix, input_vault, output_vault, user, true)? {
            Some(info) => info.into(),
            None => return Err(ParseError::NoDetailsFound),
        },
    };

    Ok(Some(DexSwap {
        user,
        market,
        program,
        swap,
    }))
}

fn parse_log<T: SolanaInstruction>(view: &IxView<T>) -> Result<Option<Event>, TxLogsError> {
    Ok(view
        .logs
        .program_logs(view.ix.path())?
        .find_map(|log| decode_log(log.strip_prefix("Program data:").map(|s| s.trim())?)))
}

fn decode_log(log: impl AsRef<str>) -> Option<Event> {
    let bytes = STANDARD.decode(log.as_ref()).ok()?;

    if bytes.len() < 8 {
        return None;
    }

    match &bytes[..8] {
        SWAP_EVENT => {
            let buf = &mut &bytes[8..];
            Event::deserialize(buf).ok()
        }
        _ => None,
    }
}

#[derive(AnchorDeserialize, Debug)]
#[allow(dead_code)]
struct Event {
    pub pool_state: Pubkey,
    pub sender: Pubkey,
    pub token_account0: Pubkey,
    pub token_account1: Pubkey,
    pub amount0: u64,
    pub transfer_fee0: u64,
    pub amount1: u64,
    pub transfer_fee1: u64,
    pub zero_for_one: bool,
    pub sqrt_price_x64: u128,
    pub liquidity: u128,
    pub tick: i32,
}
