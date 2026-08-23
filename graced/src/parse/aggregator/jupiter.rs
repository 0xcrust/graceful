use super::AggregatorSwap;
use crate::{
    parse::{IxView, ParseError},
    swap::{Program, Swap, graph::SwapGraph},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, logs::filter_cpi_logs_data},
};

declare_program!(jupiter_v6);

use anchor_lang::{Discriminator, declare_program, prelude::anchor_lang};
use borsh::BorshDeserialize;
use jupiter_v6::{
    client::args::{
        ExactOutRoute, ExactOutRouteV2, Route, RouteV2, RouteWithTokenLedger,
        SharedAccountsExactOutRoute, SharedAccountsExactOutRouteV2, SharedAccountsRoute,
        SharedAccountsRouteV2, SharedAccountsRouteWithTokenLedger,
    },
    events::{SwapEvent, SwapsEvent},
};

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<AggregatorSwap>, ParseError> {
    let IxView { ix, ref keys, .. } = view;
    let data = ix.data()?;
    if data.len() < 8 {
        return Ok(None);
    }

    let discriminator = &data[..8];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let user = *match discriminator {
        ExactOutRoute::DISCRIMINATOR => accs.peek(1)?,
        ExactOutRouteV2::DISCRIMINATOR => accs.peek(0)?,
        Route::DISCRIMINATOR => accs.peek(1)?,
        RouteV2::DISCRIMINATOR => accs.peek(0)?,
        RouteWithTokenLedger::DISCRIMINATOR => accs.peek(1)?,
        SharedAccountsExactOutRoute::DISCRIMINATOR => accs.peek(2)?,
        SharedAccountsExactOutRouteV2::DISCRIMINATOR => accs.peek(1)?,
        SharedAccountsRoute::DISCRIMINATOR => accs.peek(2)?,
        SharedAccountsRouteV2::DISCRIMINATOR => accs.peek(1)?,
        SharedAccountsRouteWithTokenLedger::DISCRIMINATOR => accs.peek(2)?,
        _ => return Ok(None),
    };

    let program = Program::JupV6;
    let program_id = program.pubkey();
    let cpi_data = filter_cpi_logs_data(keys, &program_id, ix.inner_instructions());

    let mut swaps = vec![];
    for data in cpi_data {
        if data.len() <= 8 {
            continue;
        }
        match &data[0..8] {
            SwapEvent::DISCRIMINATOR => {
                let event = SwapEvent::deserialize(&mut &data[8..])?;
                swaps.push(Swap {
                    input_amount: event.input_amount,
                    output_amount: event.output_amount,
                    input_mint: event.input_mint,
                    output_mint: event.output_mint,
                });
            }
            SwapsEvent::DISCRIMINATOR => {
                let events = SwapsEvent::deserialize(&mut &data[8..])?;
                let events: Vec<_> = events
                    .swap_events
                    .into_iter()
                    .map(|ev| Swap {
                        input_amount: ev.input_amount,
                        output_amount: ev.output_amount,
                        input_mint: ev.input_mint,
                        output_mint: ev.output_mint,
                    })
                    .collect();
                swaps = events;
                break;
            }
            _ => {}
        }
    }

    if swaps.is_empty() {
        return Ok(None);
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
