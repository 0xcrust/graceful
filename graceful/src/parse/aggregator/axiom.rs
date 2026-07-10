use super::AggregatorSwap;
use crate::{
    parse::{IxView, ParseError},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

const SWAP_DISCRIMINATOR: u8 = 0;

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<AggregatorSwap>, ParseError> {
    let IxView { ix, ref keys, .. } = view;

    let data = ix.data()?;

    let Some(disc) = data.first() else {
        return Ok(None);
    };

    if *disc != SWAP_DISCRIMINATOR {
        return Ok(None);
    }

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (input_token_account, user) = (*accs.peek(0)?, *accs.peek(1)?);

    let _input_token_account = if input_token_account == user {
        None
    } else {
        Some(input_token_account)
    };

    let routes = view.aggregator_routes()?;
    let swap = routes.swap().ok();

    let parsed = AggregatorSwap {
        program: Program::AxiomTrade,
        user,
        swap,
        routes,
    };

    Ok(Some(parsed))
}
