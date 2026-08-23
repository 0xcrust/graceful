pub mod basic;
pub mod prop;

use basic::*;
use prop::*;

use crate::{
    parse::{DexSwap, IxView, ParseError},
    swap::Program,
    transaction::instruction::SolanaInstruction,
};

pub fn is_amm(program: &Program) -> bool {
    basic::is_basic_amm(program) || prop::is_prop_amm(program)
}

pub fn parse<T: SolanaInstruction>(
    view: IxView<T>,
    program: &Program,
) -> Result<Option<DexSwap>, ParseError> {
    match program {
        Program::BonkSwap => bonkswap::parse(view),
        Program::Byreal => byreal::parse(view),
        Program::Deriverse => deriverse::parse(view),
        Program::LaunchLab => launchlab::parse(view),
        Program::Manifest => manifest::parse(view),
        Program::MeteoraDAMMV2 => meteora_damm_v2::parse(view),
        Program::MeteoraDBC => meteora_dbc::parse(view),
        Program::MeteoraDLMM => meteora_dlmm::parse(view),
        Program::MeteoraPools => meteora_pools::parse(view),
        Program::Orca => orca::parse(view),
        Program::PancakeSwap => pancake_swap::parse(view),
        Program::Pump => pump::parse(view),
        Program::PumpAmm => pump_amm::parse(view),
        Program::RaydiumClmm => raydium_clmm::parse(view),
        Program::RaydiumCpSwap => raydium_cp::parse(view),
        Program::RaydiumV4 => raydium_v4::parse(view),
        Program::SarosAmm => saros_amm::parse(view),
        Program::SarosDLMM => saros_dlmm::parse(view),
        Program::StabbleStable => stabble_stable::parse(view),
        Program::StabbleWeighted => stabble_weighted::parse(view),
        Program::GoonFi => goonfi::v1::parse(view),
        Program::GoonFiV2 => goonfi::v2::parse(view),
        Program::AlphaQ => alphaq::parse(view),
        Program::Aquifer => aquifer::parse(view),
        Program::BisonFi => bisonfi::parse(view),
        Program::HumidiFi => humidifi::parse(view),
        Program::Scorch => scorch::parse(view),
        Program::SolFi => solfi::parse(view),
        Program::SolFiV2 => solfi_v2::parse(view),
        Program::TesseraV => tessera::parse(view),
        Program::ZeroFi => zerofi::parse(view),
        _ => Ok(None),
    }
}
