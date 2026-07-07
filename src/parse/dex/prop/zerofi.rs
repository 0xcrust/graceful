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

const SWAP_V2: u8 = 16;

pub fn parse<T: SolanaInstruction>(ix: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix,
        ref keys,
        balance,
        ..
    } = ix;

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let data = ix.data()?;
    let (user, pool, pool_base_token_account, pool_quote_token_account) =
        match data.first().copied() {
            Some(SWAP_V2) => (accs[8], accs[0], accs[3], accs[5]),
            _ => (accs[7], accs[0], accs[2], accs[4]), // previous versions
        };

    let Some(swap_info) = get_swap_info(
        &balance,
        keys,
        ix,
        pool_base_token_account,
        pool_quote_token_account,
        user,
        true,
    )?
    else {
        return Ok(None);
    };

    Ok(Some(DexSwap {
        user,
        market: pool,
        program: Program::ZeroFi,
        swap: swap_info.into(),
    }))
}
