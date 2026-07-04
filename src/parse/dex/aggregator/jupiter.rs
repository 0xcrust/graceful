#![allow(unexpected_cfgs)]
#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use anchor_lang::prelude::anchor_lang;
use anchor_lang::{Discriminator, declare_program};
use borsh::BorshDeserialize;

use super::super::DexParseError;
use crate::swap::graph::SwapGraph;
use crate::swap::{Swap, SwapProgram};
use crate::util::{
    keys::{AccountKeys, InstructionAccounts},
    logs::filter_cpi_logs_data,
};
use crate::{parse::dex::aggregator::AggregatorSwap, transaction::instruction::SolanaInstruction};

declare_program!(jupiter_v6);

use jupiter_v6::{
    client::args::{
        ExactOutRoute, ExactOutRouteV2, Route, RouteV2, RouteWithTokenLedger,
        SharedAccountsExactOutRoute, SharedAccountsExactOutRouteV2, SharedAccountsRoute,
        SharedAccountsRouteV2, SharedAccountsRouteWithTokenLedger,
    },
    events::{SwapEvent, SwapsEvent},
};

pub fn parse(
    ix: &impl SolanaInstruction,
    keys: Arc<AccountKeys>,
) -> Result<Option<AggregatorSwap>, DexParseError> {
    let data = ix.data()?;
    if data.len() < 8 {
        return Ok(None);
    }

    let discriminator = &data[..8];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let user = match discriminator {
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

    let program = SwapProgram::JupV6;
    let program_id = program.pubkey();
    let cpi_data = filter_cpi_logs_data(&keys, &program_id, ix.inner_ixs());

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

    let graph = SwapGraph::new(swaps.clone());
    let swap = graph.swap();

    Ok(Some(AggregatorSwap {
        program,
        user,
        swap: Some(swap),
        swaps: Some(swaps),
    }))
}
