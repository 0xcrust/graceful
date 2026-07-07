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

    let user = *accs.peek(1)?;

    let routes = view.aggregator_routes()?;
    let swap = routes.swap();

    let parsed = AggregatorSwap {
        program: Program::AxiomTrade,
        user,
        swap,
        routes,
    };

    Ok(Some(parsed))
}
