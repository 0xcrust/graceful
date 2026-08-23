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

const SWAP_V1: u8 = 1;
const SWAP_V2: u8 = 19;

pub fn parse<T: SolanaInstruction>(ix: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix,
        ref keys,
        balance,
        ..
    } = ix;

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let data = ix.data()?;
    let (user, _base_mint, _quote_mint, pool_base_token_account, pool_quote_token_account, pool) =
        match data.first().copied() {
            Some(SWAP_V1) | Some(SWAP_V2) => {
                (accs[1], accs[7], accs[4], accs[15], accs[13], accs[9])
            }
            _ => return Ok(None),
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
        program: Program::Aquifer,
        swap: swap_info.into(),
    }))
}
