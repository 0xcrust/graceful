use super::AggregatorSwap;
use crate::{
    parse::{IxView, ParseError},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::accounts::InstructionAccounts,
};

use borsh::BorshDeserialize;

const SWAP_ROUTE_V2: &[u8] = &[249, 91, 84, 33, 69, 22, 0, 135];

const SWAP_ROUTE_V3: u8 = 42;

const _SWAP_DETAILS: &[u8] = &[101, 204, 239, 162, 252, 46, 220, 138];

const _SWAP_DETAILS_V3: &[u8] = &[79, 62, 249, 87, 62, 217, 136, 30];

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<AggregatorSwap>, ParseError> {
    let IxView { ix, ref keys, .. } = view;

    let data = ix.data()?;

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user_idx, _user_input_idx, _user_output_idx, _mints) =
        if data.len() > 8 && &data[..8] == SWAP_ROUTE_V2 {
            (0, 3, 5, Some((2, 4)))
        } else if data.first() == Some(&SWAP_ROUTE_V3) {
            (1, 3, 4, None)
        } else {
            return Ok(None);
        };

    let user = *accs.peek(user_idx)?;

    let program = Program::TitanExchangeRouter;

    let routes = view.aggregator_routes()?;
    let swap = routes.swap().ok();

    let parsed = AggregatorSwap {
        user,
        program,
        swap,
        routes,
    };

    Ok(Some(parsed))
}

#[allow(dead_code)]
#[derive(BorshDeserialize)]
struct SwapRouteV3Details {
    input_amount: u64,
    output_amount: u64,
    expected_output: u64,
    fee_collected: u64,
    surplus_collected: u64,
    dynamic_allocation_fee_collected: u64,
}

#[allow(dead_code)]
#[derive(BorshDeserialize)]
struct SwapDetails {
    index: u8,
    input_amount: u64,
    output_amount: u64,
}

// * V2 swap only returns `SwapDetails` for each CPI.
// * V3 swap returns `SwapDetails` for each CPI and then one `SwapRouteV3Details`
// describing the entire swap.
