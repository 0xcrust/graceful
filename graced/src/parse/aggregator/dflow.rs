use super::AggregatorSwap;
use crate::{
    parse::{IxView, ParseError},
    swap::{Program, Swap, graph::SwapGraph},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, logs::filter_cpi_logs_data},
};

declare_program!(dflow_agg);

use anchor_lang::{Discriminator, declare_program, prelude::anchor_lang};
use borsh::BorshDeserialize;
use dflow_agg::{
    client::args::{self, *},
    events::SwapEvent,
};

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<AggregatorSwap>, ParseError> {
    let IxView { ix, ref keys, .. } = view;
    let data = ix.data()?;
    if data.len() < 8 {
        return Ok(None);
    }

    let discriminator = &data[..8];

    if !matches!(
        discriminator,
        args::Swap::DISCRIMINATOR
            | Swap2::DISCRIMINATOR
            | Swap2WithDestination::DISCRIMINATOR
            | Swap2WithDestinationNative::DISCRIMINATOR
            | SwapWithDestination::DISCRIMINATOR
            | SwapWithDestinationNative::DISCRIMINATOR
    ) {
        return Ok(None);
    }

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;
    let user = *accs.peek(3)?;

    let program = Program::DflowAgg;
    let program_id = program.pubkey();
    let cpi_data = filter_cpi_logs_data(keys, &program_id, ix.inner_instructions());

    let mut swaps = vec![];
    for data in cpi_data {
        if data.len() >= 8 && (&data[0..8] == SwapEvent::DISCRIMINATOR) {
            let event = SwapEvent::deserialize(&mut &data[8..])?;
            swaps.push(Swap {
                input_amount: event.input_amount,
                output_amount: event.output_amount,
                input_mint: event.input_mint,
                output_mint: event.output_mint,
            });
        }
    }

    let routes = view.aggregator_routes()?;
    let swap = SwapGraph::new(swaps).swap().ok();

    Ok(Some(AggregatorSwap {
        program,
        user,
        swap,
        routes,
    }))
}
