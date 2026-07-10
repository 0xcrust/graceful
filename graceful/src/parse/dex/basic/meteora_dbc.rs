use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
const SWAP2: &[u8] = &[65, 75, 63, 76, 235, 91, 91, 136];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user, market, base_vault, quote_vault) = match discriminator {
        SWAP | SWAP2 => (accs[9], accs[2], accs[5], accs[6]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, base_vault, quote_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::Byreal,
        swap: info.into(),
    }))
}
