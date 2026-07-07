use crate::{
    parse::{DexSwap, IxView, ParseError},
    swap::{Program, Swap},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, balances::Addr},
};

use spl_token_interface::native_mint;

const BUY: &[u8] = &[102, 6, 61, 18, 1, 218, 235, 234];

const BUY_EXACT_SOL_IN: &[u8] = &[56, 252, 116, 8, 158, 223, 205, 95];

const SELL: &[u8] = &[51, 230, 133, 164, 1, 127, 131, 173];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (base_mint, base_vault, bonding_curve, user) = match discriminator {
        BUY | BUY_EXACT_SOL_IN | SELL => (accs[2], accs[4], accs[3], accs[6]),
        _ => return Ok(None),
    };

    let quote_mint = native_mint::ID;

    let base_diff = balance
        .find_token_account_balance(Addr::Key(base_vault))
        .difference()?;
    let quote_diff = balance
        .find_native_balance(Addr::Key(bonding_curve))
        .difference()?;

    let base_for_quote = match (base_diff.is_positive(), quote_diff.is_positive()) {
        (true, false) => true,
        (false, true) => false,
        _ => return Err(ParseError::NoDetailsFound),
    };

    let swap = match base_for_quote {
        true => Swap {
            input_amount: base_diff.abs().try_into().unwrap(),
            input_mint: base_mint,
            output_amount: quote_diff.abs().try_into().unwrap(),
            output_mint: quote_mint,
        },
        false => Swap {
            input_amount: quote_diff.abs().try_into().unwrap(),
            input_mint: quote_mint,
            output_amount: base_diff.abs().try_into().unwrap(),
            output_mint: base_mint,
        },
    };

    Ok(Some(DexSwap {
        user,
        market: bonding_curve,
        program: Program::Pump,
        swap,
    }))
}
