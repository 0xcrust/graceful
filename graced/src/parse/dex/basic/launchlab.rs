use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const BUY_EXACT_IN: &[u8] = &[250, 234, 13, 123, 213, 156, 19, 236];
const BUY_EXACT_OUT: &[u8] = &[24, 211, 116, 40, 105, 3, 153, 56];
const SELL_EXACT_IN: &[u8] = &[149, 39, 222, 155, 211, 124, 152, 26];
const SELL_EXACT_OUT: &[u8] = &[95, 200, 71, 34, 8, 9, 11, 166];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (base_vault, quote_vault, market, user) = match discriminator {
        BUY_EXACT_IN | BUY_EXACT_OUT | SELL_EXACT_IN | SELL_EXACT_OUT => {
            (accs[7], accs[8], accs[4], accs[0])
        }
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, base_vault, quote_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::LaunchLab,
        swap: info.into(),
    }))
}
