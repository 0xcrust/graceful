use super::AggregatorSwap;
use crate::{
    parse::{IxView, ParseError},
    swap::{Program, Swap},
    transaction::instruction::SolanaInstruction,
    util::{accounts::InstructionAccounts, logs::filter_cpi_logs_data},
};

declare_program!(okx_dex_router);

use anchor_lang::{Discriminator, declare_program, prelude::anchor_lang};
use borsh::BorshDeserialize;
use okx_dex_router::{client::args, events::*};

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<AggregatorSwap>, ParseError> {
    let IxView { ix, ref keys, .. } = view;
    let data = ix.data()?;
    if data.len() < 8 {
        return Ok(None);
    }

    let discriminator = &data[..8];
    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    if !matches!(
        discriminator,
        args::Swap::DISCRIMINATOR
            | args::SwapTob::DISCRIMINATOR
            | args::SwapTobEnhanced::DISCRIMINATOR
            | args::SwapTobV2::DISCRIMINATOR
            | args::SwapTobWithReceiver::DISCRIMINATOR
            | args::SwapTobWithReceiverTokenLedger::DISCRIMINATOR
            | args::SwapTobWithTokenLedger::DISCRIMINATOR
            | args::SwapToc::DISCRIMINATOR
            | args::SwapTocV2::DISCRIMINATOR
    ) {
        return Ok(None);
    }

    let user = *accs.peek(0)?;

    let program = Program::OKXDexRouter2;
    let program_id = program.pubkey();
    let cpi_data = filter_cpi_logs_data(keys, &program_id, ix.inner_instructions());

    let mut swap = None;

    for data in cpi_data {
        if data.len() <= 8 {
            continue;
        }

        let discriminator = &data[0..8];
        let mut rest = &data[8..];

        match discriminator {
            SwapWithFeesCpiEvent::DISCRIMINATOR => {
                let event = SwapWithFeesCpiEvent::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            SwapWithFeesCpiEvent2::DISCRIMINATOR => {
                let event = SwapWithFeesCpiEvent2::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            SwapWithFeesCpiEventEnhanced::DISCRIMINATOR => {
                let event = SwapWithFeesCpiEventEnhanced::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            SwapWithFeesCpiEventEnhanced2::DISCRIMINATOR => {
                let event = SwapWithFeesCpiEventEnhanced2::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            SwapCpiEvent::DISCRIMINATOR => {
                let event = SwapCpiEvent::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            SwapCpiEvent2::DISCRIMINATOR => {
                let event = SwapCpiEvent2::deserialize(&mut rest)?;
                swap = Some(Swap {
                    input_amount: event.source_token_change,
                    output_amount: event.destination_token_change,
                    input_mint: event.source_mint,
                    output_mint: event.destination_mint,
                });
                break;
            }
            _ => {}
        }
    }

    let parsed = AggregatorSwap {
        program,
        user,
        swap,
        routes: view.aggregator_routes()?,
    };

    Ok(Some(parsed))
}
