pub mod bonkswap;
pub mod byreal;
pub mod deriverse;
pub mod launchlab;
pub mod manifest;
pub mod meteora_damm_v2;
pub mod meteora_dbc;
pub mod meteora_dlmm;
pub mod meteora_pools;
pub mod orca;
pub mod pancake_swap;
pub mod pump;
pub mod pump_amm;
pub mod raydium_clmm;
pub mod raydium_cp;
pub mod raydium_v4;
pub mod saros_amm;
pub mod saros_dlmm;
pub mod stabble_stable;
pub mod stabble_weighted;

use crate::swap::Program;

pub fn is_basic_amm(program: &Program) -> bool {
    matches!(
        program,
        Program::BonkSwap
            | Program::Byreal
            | Program::Deriverse
            | Program::LaunchLab
            | Program::Manifest
            | Program::MeteoraDAMMV2
            | Program::MeteoraDBC
            | Program::MeteoraDLMM
            | Program::MeteoraPools
            | Program::Orca
            | Program::PancakeSwap
            | Program::Pump
            | Program::PumpAmm
            | Program::RaydiumClmm
            | Program::RaydiumCpSwap
            | Program::RaydiumV4
            | Program::SarosAmm
            | Program::SarosDLMM
            | Program::StabbleStable
            | Program::StabbleWeighted
    )
}
