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

pub fn parse<T: SolanaInstruction>(ix: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix,
        ref keys,
        balance,
        ..
    } = ix;

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    // Both V1 & V2
    let (user, pool, pool_base_token_account, pool_quote_token_account) =
        (accs[0], accs[1], accs[2], accs[3]);

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
        program: Program::HumidiFi,
        swap: swap_info.into(),
    }))
}
