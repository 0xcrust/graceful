use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: u8 = 26;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let Some(discriminator) = data.first() else {
        return Ok(None);
    };

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user, _asset_mint, _curr_mint, asset_vault, curr_vault, instr_acc) = match *discriminator {
        SWAP => (accs[0], accs[1], accs[2], accs[3], accs[4], accs[5]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, asset_vault, curr_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market: instr_acc,
        program: Program::Deriverse,
        swap: info.into(),
    }))
}
