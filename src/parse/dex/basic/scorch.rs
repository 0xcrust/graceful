use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: u8 = 2;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let data = ix.data()?;
    let (user, market, vault_in, vault_out, _token_in_mint, _token_out_mint) =
        match data.get(0).copied() {
            Some(SWAP) => (accs[1], accs[15], accs[4], accs[5], accs[6], accs[7]),
            _ => return Ok(None),
        };

    let info = get_swap_info(&balance, &keys, ix, vault_in, vault_out, user, true)?
        .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::Scorch,
        swap: info.into(),
    }))
}
