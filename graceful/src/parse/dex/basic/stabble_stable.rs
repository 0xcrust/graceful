use solana_pubkey::{Pubkey, pubkey};

use crate::{
    parse::{DexSwap, IxView, ParseError, util::get_swap_info_from_token_transfers},
    swap::Program,
    transaction::instruction::SolanaInstruction,
    util::{
        accounts::InstructionAccounts,
        transfer::{parse_multiple_token_transfers, parse_token_transfer},
    },
};

const SWAP: &[u8] = &[248, 198, 158, 145, 225, 117, 135, 200];
const SWAP_V2: &[u8] = &[43, 4, 237, 11, 26, 201, 30, 98];

const VAULT_PROGRAM: Pubkey = pubkey!("vo1tWgqZMjG61Z2T9qUaMYKqZ75CYzMuaZ2LZP1n7HV");

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let IxView {
        ix, keys, balance, ..
    } = view;

    let data = ix.data()?;
    let discriminator = &data[..std::cmp::min(8, data.len())];

    let accs = InstructionAccounts::new(keys.clone(), ix.accounts(), ix.program_id())?;

    let (user, market, input_vault, output_vault) = match discriminator {
        SWAP => (accs[0], accs[6], accs[3], accs[4]),
        SWAP_V2 => (accs[0], accs[8], accs[5], accs[6]),
        _ => return Ok(None),
    };

    let mut transfers = parse_multiple_token_transfers(&keys, ix.inner_instructions())?;

    let withdraw_ix = ix
        .inner_instructions()
        .find_map(|ix| {
            let program_id = keys.get(ix.program_id())?;
            if *program_id == VAULT_PROGRAM {
                parse_token_transfer(&keys, ix.accounts(), &ix.data().ok()?, ix.program_id())
                    .ok()?
            } else {
                None
            }
        })
        .ok_or(ParseError::NoDetailsFound)?;
    transfers.push(withdraw_ix);

    let info = get_swap_info_from_token_transfers(
        &balance,
        transfers,
        input_vault,
        output_vault,
        user,
        true,
    )?
    .ok_or(ParseError::NoDetailsFound)?;

    Ok(Some(DexSwap {
        user,
        market,
        program: Program::StabbleStable,
        swap: info.into(),
    }))
}
