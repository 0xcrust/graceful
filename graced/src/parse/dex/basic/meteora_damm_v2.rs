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

    let (market, user, _token_a, _token_b, vault_a, vault_b) = match discriminator {
        SWAP | SWAP2 => (accs[1], accs[8], accs[6], accs[7], accs[4], accs[5]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, vault_a, vault_b, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::MeteoraDAMMV2,
        swap: info.into(),
    }))
}
