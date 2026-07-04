pub mod graph;

use std::collections::HashSet;

use lazy_static::lazy_static;
use solana_pubkey::{Pubkey, pubkey};

lazy_static! {
    pub static ref DISALLOWED: HashSet<Pubkey> = HashSet::from_iter([
        spl_token_interface::ID,
        spl_token_2022_interface::ID,
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"), // Associated token
        solana_sdk_ids::compute_budget::ID,
        solana_sdk_ids::system_program::ID,
        solana_sdk_ids::vote::ID,
    ]);
}

/// The base swap type.
///
/// A single elementary swap: `input_amount` of `input_mint` was exchanged for
/// `output_amount` of `output_mint`.
///
/// This is typically one leg of a route as reported by a swap program or aggregator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Swap {
    /// The input amount.
    pub input_amount: u64,
    /// The output amount.
    pub output_amount: u64,
    /// The input token.
    pub input_mint: Pubkey,
    /// The output token.
    pub output_mint: Pubkey,
}

impl Swap {
    /// Creates a new elementary swap record.
    pub fn new(
        input_mint: Pubkey,
        output_mint: Pubkey,
        input_amount: u64,
        output_amount: u64,
    ) -> Self {
        Self {
            input_mint,
            output_mint,
            input_amount,
            output_amount,
        }
    }

    /// Returns `true` if `mint` is either the input or the output side of this swap.
    pub fn has_mint(&self, mint: &Pubkey) -> bool {
        self.input_mint == *mint || self.output_mint == *mint
    }
}

/// Swap with decimals.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SwapWithDecimals {
    pub base: Swap,
    pub input_decimals: u8,
    pub output_decimals: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SwapProgram {
    AldrinV1,
    AldrinV2,
    AlphaQ,
    Aquifer,
    AxiomTrade,
    BinanceWalletProgram,
    BisonFi,
    BonkSwap,
    Byreal,
    DflowAgg,
    Drift,
    GMGN,
    GMX,
    GoonFi,
    GoonFiV2,
    GooseFx,
    Heaven,
    HumidiFi,
    JupV6,
    LaunchLab,
    LifinityV2,
    MangoV4,
    Manifest,
    MeteoraDBC,
    MeteoraDLMM,
    MeteoraDAMMV2,
    MeteoraPools,
    MercurialStableSwap,
    ObricV2,
    OKXRouter,
    OKXDexRouter2,
    Orca,
    PancakeSwap,
    Pheonix,
    Pump,
    PumpAmm,
    RaydiumClmm,
    RaydiumCpSwap,
    RaydiumV4,
    SarosDLMM,
    SolFi,
    SolFiV2,
    Stabble,
    StabbleWeightedSwap,
    TesseraV,
    TitanExchangeRouter,
    ZeroFi,
    Unknown(Pubkey),
}

impl SwapProgram {
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self)
    }
}

impl Default for SwapProgram {
    fn default() -> Self {
        SwapProgram::Unknown(Pubkey::default())
    }
}

impl From<SwapProgram> for Pubkey {
    fn from(value: SwapProgram) -> Self {
        Pubkey::from(&value)
    }
}

impl From<&SwapProgram> for Pubkey {
    fn from(value: &SwapProgram) -> Self {
        match value {
            SwapProgram::AldrinV1 => pubkey!("AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6"),
            SwapProgram::AldrinV2 => pubkey!("CURVGoZn8zycx6FXwwevgBTB2gVvdbGTEpvMJDbgs2t4"),
            SwapProgram::AlphaQ => pubkey!("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA"),
            SwapProgram::Aquifer => pubkey!("AQU1FRd7papthgdrwPTTq5JacJh8YtwEXaBfKU3bTz45"),
            SwapProgram::AxiomTrade => pubkey!("FLASHX8DrLbgeR8FcfNV1F5krxYcYMUdBkrP1EPBtxB9"),
            SwapProgram::BinanceWalletProgram => {
                pubkey!("B3111yJCeHBcA1bizdJjUFPALfhAfSRnAbJzGUtnt56A")
            }
            SwapProgram::BisonFi => pubkey!("BiSoNHVpsVZW2F7rx2eQ59yQwKxzU5NvBcmKshCSUypi"),
            SwapProgram::BonkSwap => pubkey!("BSwp6bEBihVLdqJRKGgzjcGLHkcTuzmSo1TQkHepzH8p"),
            SwapProgram::Byreal => pubkey!("REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2"),
            SwapProgram::DflowAgg => pubkey!("DF1ow4tspfHX9JwWJsAb9epbkA8hmpSEAtxXy1V27QBH"),
            SwapProgram::Drift => pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"),
            SwapProgram::GoonFi => pubkey!("goonERTdGsjnkZqWuVjs73BZ3Pb9qoCUdBUL17BnS5j"),
            SwapProgram::GoonFiV2 => pubkey!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE"),
            SwapProgram::GMGN => pubkey!("GMgnVFR8Jb39LoXsEVzb3DvBy3ywCmdmJquHUy1Lrkqb"),
            SwapProgram::GMX => pubkey!("Gmso1uvJnLbawvw7yezdfCDcPydwW2s2iqG3w6MDucLo"),
            SwapProgram::GooseFx => pubkey!("GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT"),
            SwapProgram::Heaven => pubkey!("HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o"),
            SwapProgram::HumidiFi => pubkey!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp"),
            SwapProgram::JupV6 => pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"),
            SwapProgram::LaunchLab => pubkey!("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj"),
            SwapProgram::LifinityV2 => pubkey!("2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c"),
            SwapProgram::MangoV4 => pubkey!("4MangoMjqJ2firMokCjjGgoK8d4MXcrgL7XJaL3w6fVg"),
            SwapProgram::Manifest => pubkey!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms"),
            SwapProgram::MercurialStableSwap => {
                pubkey!("MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky")
            }
            SwapProgram::MeteoraDBC => pubkey!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN"),
            SwapProgram::MeteoraPools => pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB"),
            SwapProgram::MeteoraDAMMV2 => pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG"),
            SwapProgram::MeteoraDLMM => pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo"),
            SwapProgram::ObricV2 => pubkey!("obriQD1zbpyLz95G5n7nJe6a4DPjpFwa5XYPoNm113y"),
            SwapProgram::OKXRouter => pubkey!("6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma"),
            SwapProgram::OKXDexRouter2 => pubkey!("proVF4pMXVaYqmy4NjniPh4pqKNfMmsihgd4wdkCX3u"),
            SwapProgram::Orca => pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"),
            SwapProgram::PancakeSwap => pubkey!("HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq"),
            SwapProgram::Pheonix => pubkey!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"),
            SwapProgram::Pump => pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"),
            SwapProgram::PumpAmm => pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"),
            SwapProgram::RaydiumClmm => pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK"),
            SwapProgram::RaydiumCpSwap => pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C"),
            SwapProgram::RaydiumV4 => pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"),
            SwapProgram::SarosDLMM => pubkey!("1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE"),
            SwapProgram::SolFi => pubkey!("SoLFiHG9TfgtdUXUjWAxi3LtvYuFyDLVhBWxdMZxyCe"),
            SwapProgram::SolFiV2 => pubkey!("SV2EYYJyRz2YhfXwXnhNAevDEui5Q6yrfyo13WtupPF"),
            SwapProgram::Stabble => pubkey!("swapNyd8XiQwJ6ianp9snpu4brUqFxadzvHebnAXjJZ"),
            SwapProgram::StabbleWeightedSwap => {
                pubkey!("swapFpHZwjELNnjvThjajtiVmkz3yPQEHjLtka2fwHW")
            }
            SwapProgram::TesseraV => pubkey!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH"),
            SwapProgram::TitanExchangeRouter => {
                pubkey!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT")
            }
            SwapProgram::ZeroFi => pubkey!("ZERor4xhbUycZ6gb9ntrhqscUcZmAbQDjEAtCf4hbZY"),
            SwapProgram::Unknown(pubkey) => *pubkey,
        }
    }
}

impl From<Pubkey> for SwapProgram {
    fn from(pubkey: Pubkey) -> Self {
        use SwapProgram::*;
        match pubkey.to_string().as_str() {
            "AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6" => AldrinV1,
            "CURVGoZn8zycx6FXwwevgBTB2gVvdbGTEpvMJDbgs2t4" => AldrinV2,
            "ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA" => AlphaQ,
            "AQU1FRd7papthgdrwPTTq5JacJh8YtwEXaBfKU3bTz45" => Aquifer,
            "FLASHX8DrLbgeR8FcfNV1F5krxYcYMUdBkrP1EPBtxB9" => AxiomTrade,
            "B3111yJCeHBcA1bizdJjUFPALfhAfSRnAbJzGUtnt56A" => BinanceWalletProgram,
            "BiSoNHVpsVZW2F7rx2eQ59yQwKxzU5NvBcmKshCSUypi" => BisonFi,
            "BSwp6bEBihVLdqJRKGgzjcGLHkcTuzmSo1TQkHepzH8p" => BonkSwap,
            "REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2" => Byreal,
            "DF1ow4tspfHX9JwWJsAb9epbkA8hmpSEAtxXy1V27QBH" => DflowAgg,
            "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH" => Drift,
            "goonERTdGsjnkZqWuVjs73BZ3Pb9qoCUdBUL17BnS5j" => GoonFi,
            "goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE" => GoonFiV2,
            "GMgnVFR8Jb39LoXsEVzb3DvBy3ywCmdmJquHUy1Lrkqb" => GMGN,
            "Gmso1uvJnLbawvw7yezdfCDcPydwW2s2iqG3w6MDucLo" => GMX,
            "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT" => GooseFx,
            "HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o" => Heaven,
            "9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp" => HumidiFi,
            "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4" => JupV6,
            "LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj" => LaunchLab,
            "2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c" => LifinityV2,
            "4MangoMjqJ2firMokCjjGgoK8d4MXcrgL7XJaL3w6fVg" => MangoV4,
            "MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms" => Manifest,
            "MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky" => MercurialStableSwap,
            "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB" => MeteoraPools,
            "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG" => MeteoraDAMMV2,
            "dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN" => MeteoraDBC,
            "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo" => MeteoraDLMM,
            "obriQD1zbpyLz95G5n7nJe6a4DPjpFwa5XYPoNm113y" => ObricV2,
            "6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma" => OKXRouter,
            "proVF4pMXVaYqmy4NjniPh4pqKNfMmsihgd4wdkCX3u" => OKXDexRouter2,
            "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc" => Orca,
            "HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq" => PancakeSwap,
            "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY" => Pheonix,
            "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P" => Pump,
            "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA" => PumpAmm,
            "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK" => RaydiumClmm,
            "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C" => RaydiumCpSwap,
            "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8" => RaydiumV4,
            "1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE" => SarosDLMM,
            "SoLFiHG9TfgtdUXUjWAxi3LtvYuFyDLVhBWxdMZxyCe" => SolFi,
            "SV2EYYJyRz2YhfXwXnhNAevDEui5Q6yrfyo13WtupPF" => SolFiV2,
            "swapNyd8XiQwJ6ianp9snpu4brUqFxadzvHebnAXjJZ" => Stabble,
            "swapFpHZwjELNnjvThjajtiVmkz3yPQEHjLtka2fwHW" => StabbleWeightedSwap,
            "TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH" => TesseraV,
            "T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT" => TitanExchangeRouter,
            "ZERor4xhbUycZ6gb9ntrhqscUcZmAbQDjEAtCf4hbZY" => ZeroFi,
            _ => Unknown(pubkey),
        }
    }
}
