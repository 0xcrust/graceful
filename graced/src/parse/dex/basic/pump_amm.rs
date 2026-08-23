use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const BUY: &[u8] = &[102, 6, 61, 18, 1, 218, 235, 234];

const BUY_EXACT_QUOTE_IN: &[u8] = &[198, 46, 21, 82, 180, 217, 232, 112];

const SELL: &[u8] = &[51, 230, 133, 164, 1, 127, 131, 173];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (_base_mint, _quote_mint, base_vault, quote_vault, market, user) = match discriminator {
        BUY | BUY_EXACT_QUOTE_IN | SELL => (accs[3], accs[4], accs[7], accs[8], accs[0], accs[1]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, base_vault, quote_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::PumpAmm,
        swap: info.into(),
    }))
}
