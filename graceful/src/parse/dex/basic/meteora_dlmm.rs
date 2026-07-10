use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
const SWAP2: &[u8] = &[65, 75, 63, 76, 235, 91, 91, 136];
const SWAP_EXACT_OUT: &[u8] = &[250, 73, 101, 33, 38, 207, 75, 184];
const SWAP_EXACT_OUT2: &[u8] = &[43, 215, 247, 132, 137, 60, 243, 81];
const SWAP_WITH_PRICE_IMPACT: &[u8] = &[56, 173, 230, 208, 173, 228, 156, 205];
const SWAP_WITH_PRICE_IMPACT2: &[u8] = &[74, 98, 192, 214, 177, 51, 75, 51];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (market, user, _token_a, _token_b, vault_a, vault_b) = match discriminator {
        SWAP
        | SWAP2
        | SWAP_EXACT_OUT
        | SWAP_EXACT_OUT2
        | SWAP_WITH_PRICE_IMPACT
        | SWAP_WITH_PRICE_IMPACT2 => (accs[0], accs[10], accs[6], accs[7], accs[2], accs[3]),
        _ => return Ok(None),
    };

    let info = get_swap_info(&balance, &keys, ix, vault_a, vault_b, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::MeteoraDLMM,
        swap: info.into(),
    }))
}
