use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (_mints, input_vault, output_vault, market, user) = match discriminator {
        SWAP => (None, accs[5], accs[6], accs[2], accs[0]),
        SWAP_V2 => (
            Some((accs[11], accs[12])),
            accs[5],
            accs[6],
            accs[2],
            accs[0],
        ),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, input_vault, output_vault, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::Byreal,
        swap: info.into(),
    }))
}
