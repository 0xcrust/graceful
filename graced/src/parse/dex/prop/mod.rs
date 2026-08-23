pub mod alphaq;
pub mod aquifer;
pub mod bisonfi;
pub mod goonfi;
pub mod humidifi;
pub mod solfi;
pub mod solfi_v2;
pub mod tessera;
pub mod zerofi;

use crate::swap::Program;

pub fn is_prop_amm(program: &Program) -> bool {
    matches!(
        program,
        Program::AlphaQ
            | Program::Aquifer
            | Program::BisonFi
            | Program::GoonFi
            | Program::GoonFiV2
            | Program::HumidiFi
            | Program::SolFiV2
            | Program::TesseraV
            | Program::ZeroFi
    )
}
