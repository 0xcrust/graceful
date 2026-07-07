use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: u8 = 1;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let Some(discriminator) = data.first() else {
        return Ok(None);
    };

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (market, source_vault, dest_vault, user) = match *discriminator {
        SWAP => (accs[2], accs[4], accs[5], accs[9]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, source_vault, dest_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::SarosDLMM,
        swap: info.into(),
    }))
}
