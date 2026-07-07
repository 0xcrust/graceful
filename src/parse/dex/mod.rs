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
    ix: IxView<T>,
    program: &Program,
) -> Result<Option<DexSwap>, ParseError> {
    match program {
        Program::BonkSwap => bonkswap::parse(ix),
        Program::Byreal => byreal::parse(ix),
        Program::Deriverse => deriverse::parse(ix),
        Program::LaunchLab => launchlab::parse(ix),
        Program::Manifest => manifest::parse(ix),
        Program::MeteoraDAMMV2 => meteora_damm_v2::parse(ix),
        Program::MeteoraDBC => meteora_dbc::parse(ix),
        Program::MeteoraDLMM => meteora_dlmm::parse(ix),
        Program::MeteoraPools => meteora_pools::parse(ix),
        Program::Orca => orca::parse(ix),
        Program::PancakeSwap => pancake_swap::parse(ix),
        Program::Pump => pump::parse(ix),
        Program::PumpAmm => pump_amm::parse(ix),
        Program::RaydiumClmm => raydium_clmm::parse(ix),
        Program::RaydiumCpSwap => raydium_cp::parse(ix),
        Program::RaydiumV4 => raydium_v4::parse(ix),
        Program::SarosAmm => saros_amm::parse(ix),
        Program::SarosDLMM => saros_dlmm::parse(ix),
        Program::StabbleStable => stabble_stable::parse(ix),
        Program::StabbleWeighted => stabble_weighted::parse(ix),
        Program::GoonFi => goonfi::v1::parse(ix),
        Program::GoonFiV2 => goonfi::v2::parse(ix),
        Program::AlphaQ => alphaq::parse(ix),
        Program::Aquifer => aquifer::parse(ix),
        Program::BisonFi => bisonfi::parse(ix),
        Program::HumidiFi => humidifi::parse(ix),
        Program::SolFi => solfi::parse(ix),
        Program::SolFiV2 => solfi_v2::parse(ix),
        Program::TesseraV => tessera::parse(ix),
        Program::ZeroFi => zerofi::parse(ix),
        _ => Ok(None),
    }
}
