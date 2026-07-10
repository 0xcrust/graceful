use crate::{
    parse::{DexSwap, IxView, ParseError},
    swap::Program,
    transaction::instruction::SolanaInstruction,
};

pub fn parse<T: SolanaInstruction>(view: IxView<T>) -> Result<Option<DexSwap>, ParseError> {
    let mut swap = super::raydium_clmm::parse(view)?;

    if let Some(swap) = swap.as_mut() {
        swap.program = Program::PancakeSwap;
    }

    Ok(swap)
}
