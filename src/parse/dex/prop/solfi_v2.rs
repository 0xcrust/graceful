use crate::{
    parse::{
        IxView,
        dex::{DexSwap, ParseError},
        util::get_swap_info,
    },
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP: u8 = 7;

pub fn parse<T: SolanaInstruction>(ix: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix,
        ref keys,
        balance,
        ..
    } = ix;

    let data = ix.data()?;
    if data.first().copied() != Some(SWAP) {
        return Ok(None);
    }

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user, pair, pool_token_a, pool_token_b) = (accs[0], accs[1], accs[4], accs[5]);

    let Some(swap_info) =
        get_swap_info(&balance, keys, ix, pool_token_a, pool_token_b, user, true)?
    else {
        return Ok(None);
    };

    Ok(Some(DexSwap {
        user,
        market: pair,
        program: Program::SolFiV2,
        swap: swap_info.into(),
    }))
}
