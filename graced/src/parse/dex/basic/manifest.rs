use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: u8 = 4;
const SWAP_V2: u8 = 13;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let Some(discriminator) = data.first() else {
        return Ok(None);
    };

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user, market, base_vault, quote_vault) = match *discriminator {
        SWAP => (accs[0], accs[1], accs[5], accs[6]),
        SWAP_V2 => (accs[0], accs[2], accs[6], accs[7]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, base_vault, quote_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::Manifest,
        swap: info.into(),
    }))
}
