use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (market, _mint_x, _mint_y, vault_x, vault_y, user) = match discriminator {
        SWAP => (accs[0], accs[1], accs[2], accs[5], accs[6], accs[9]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, vault_x, vault_y, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::SarosDLMM,
        swap: info.into(),
    }))
}
