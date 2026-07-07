use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::LazyLock,
};

use ids::*;
use solana_pubkey::{Pubkey, pubkey};

pub static DISALLOWED: LazyLock<HashSet<Pubkey>> = LazyLock::new(|| {
    HashSet::from_iter([
        spl_token_interface::ID,
        spl_token_2022_interface::ID,
        pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"), // Associated token
        solana_sdk_ids::compute_budget::ID,
        solana_sdk_ids::system_program::ID,
        solana_sdk_ids::vote::ID,
    ])
});

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Program {
    AldrinV1,
    AldrinV2,
    AlphaQ,
    Aquifer,
    AxiomTrade,
    BinanceWalletProgram,
    BisonFi,
    BonkSwap,
    Byreal,
    Deriverse,
    DflowAgg,
    Drift,
    Gmgn,
    Gmx,
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
    Phoenix,
    Pump,
    PumpAmm,
    RaydiumClmm,
    RaydiumCpSwap,
    RaydiumV4,
    SarosAmm,
    SarosDLMM,
    Scorch,
    SolFi,
    SolFiV2,
    StabbleStable,
    StabbleWeighted,
    TesseraV,
    TitanExchangeRouter,
    ZeroFi,
    Unknown(Pubkey),
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Program::Unknown(key) => write!(f, "{}", key),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl Program {
    pub fn pubkey(&self) -> Pubkey {
        Pubkey::from(self)
    }
}

impl Default for Program {
    fn default() -> Self {
        Program::Unknown(Pubkey::default())
    }
}

impl From<&Pubkey> for Program {
    fn from(pubkey: &Pubkey) -> Self {
        PROGRAM_MAP
            .get(pubkey)
            .copied()
            .unwrap_or(Program::Unknown(*pubkey))
    }
}

impl From<Pubkey> for Program {
    fn from(value: Pubkey) -> Self {
        Program::from(&value)
    }
}

impl From<Program> for Pubkey {
    fn from(value: Program) -> Self {
        Pubkey::from(&value)
    }
}

impl From<&Program> for Pubkey {
    fn from(value: &Program) -> Self {
        match value {
            Program::AldrinV1 => ALDRIN_V1,
            Program::AldrinV2 => ALDRIN_V2,
            Program::AlphaQ => ALPHA_Q,
            Program::Aquifer => AQUIFER,
            Program::AxiomTrade => AXIOM_TRADE,
            Program::BinanceWalletProgram => BINANCE_WALLET_PROGRAM,
            Program::BisonFi => BISON_FI,
            Program::BonkSwap => BONK_SWAP,
            Program::Byreal => BYREAL,
            Program::Deriverse => DERIVERSE,
            Program::DflowAgg => DFLOW_AGG,
            Program::Drift => DRIFT,
            Program::GoonFi => GOON_FI,
            Program::GoonFiV2 => GOON_FI_V2,
            Program::Gmgn => GMGN,
            Program::Gmx => GMX,
            Program::GooseFx => GOOSE_FX,
            Program::Heaven => HEAVEN,
            Program::HumidiFi => HUMIDI_FI,
            Program::JupV6 => JUP_V6,
            Program::LaunchLab => LAUNCH_LAB,
            Program::LifinityV2 => LIFINITY_V2,
            Program::MangoV4 => MANGO_V4,
            Program::Manifest => MANIFEST,
            Program::MercurialStableSwap => MERCURIAL_STABLE_SWAP,
            Program::MeteoraDBC => METEORA_DBC,
            Program::MeteoraPools => METEORA_POOLS,
            Program::MeteoraDAMMV2 => METEORA_DAMM_V2,
            Program::MeteoraDLMM => METEORA_DLMM,
            Program::ObricV2 => OBRIC_V2,
            Program::OKXRouter => OKX_ROUTER,
            Program::OKXDexRouter2 => OKX_DEX_ROUTER_2,
            Program::Orca => ORCA,
            Program::PancakeSwap => PANCAKE_SWAP,
            Program::Phoenix => PHOENIX,
            Program::Pump => PUMP,
            Program::PumpAmm => PUMP_AMM,
            Program::RaydiumClmm => RAYDIUM_CLMM,
            Program::RaydiumCpSwap => RAYDIUM_CP_SWAP,
            Program::RaydiumV4 => RAYDIUM_V4,
            Program::SarosAmm => SAROS_AMM,
            Program::SarosDLMM => SAROS_DLMM,
            Program::Scorch => SCORCH,
            Program::SolFi => SOL_FI,
            Program::SolFiV2 => SOL_FI_V2,
            Program::StabbleStable => STABBLE_STABLE,
            Program::StabbleWeighted => STABBLE_WEIGHTED_SWAP,
            Program::TesseraV => TESSERA_V,
            Program::TitanExchangeRouter => TITAN_EXCHANGE_ROUTER,
            Program::ZeroFi => ZERO_FI,
            Program::Unknown(pubkey) => *pubkey,
        }
    }
}

static PROGRAM_MAP: LazyLock<HashMap<Pubkey, Program>> = LazyLock::new(|| {
    use Program::*;
    HashMap::from([
        (ALDRIN_V1, AldrinV1),
        (ALDRIN_V2, AldrinV2),
        (ALPHA_Q, AlphaQ),
        (AQUIFER, Aquifer),
        (AXIOM_TRADE, AxiomTrade),
        (BINANCE_WALLET_PROGRAM, BinanceWalletProgram),
        (BISON_FI, BisonFi),
        (BONK_SWAP, BonkSwap),
        (BYREAL, Byreal),
        (DERIVERSE, Deriverse),
        (DFLOW_AGG, DflowAgg),
        (DRIFT, Drift),
        (GOON_FI, GoonFi),
        (GOON_FI_V2, GoonFiV2),
        (GMGN, Gmgn),
        (GMX, Gmx),
        (GOOSE_FX, GooseFx),
        (HEAVEN, Heaven),
        (HUMIDI_FI, HumidiFi),
        (JUP_V6, JupV6),
        (LAUNCH_LAB, LaunchLab),
        (LIFINITY_V2, LifinityV2),
        (MANGO_V4, MangoV4),
        (MANIFEST, Manifest),
        (MERCURIAL_STABLE_SWAP, MercurialStableSwap),
        (METEORA_POOLS, MeteoraPools),
        (METEORA_DAMM_V2, MeteoraDAMMV2),
        (METEORA_DBC, MeteoraDBC),
        (METEORA_DLMM, MeteoraDLMM),
        (OBRIC_V2, ObricV2),
        (OKX_ROUTER, OKXRouter),
        (OKX_DEX_ROUTER_2, OKXDexRouter2),
        (ORCA, Orca),
        (PANCAKE_SWAP, PancakeSwap),
        (PHOENIX, Phoenix),
        (PUMP, Pump),
        (PUMP_AMM, PumpAmm),
        (RAYDIUM_CLMM, RaydiumClmm),
        (RAYDIUM_CP_SWAP, RaydiumCpSwap),
        (RAYDIUM_V4, RaydiumV4),
        (SAROS_AMM, SarosAmm),
        (SAROS_DLMM, SarosDLMM),
        (SCORCH, Scorch),
        (SOL_FI, SolFi),
        (SOL_FI_V2, SolFiV2),
        (STABBLE_STABLE, StabbleStable),
        (STABBLE_WEIGHTED_SWAP, StabbleWeighted),
        (TESSERA_V, TesseraV),
        (TITAN_EXCHANGE_ROUTER, TitanExchangeRouter),
        (ZERO_FI, ZeroFi),
    ])
});

mod ids {
    use solana_pubkey::{Pubkey, pubkey};

    pub const ALDRIN_V1: Pubkey = pubkey!("AMM55ShdkoGRB5jVYPjWziwk8m5MpwyDgsMWHaMSQWH6");
    pub const ALDRIN_V2: Pubkey = pubkey!("CURVGoZn8zycx6FXwwevgBTB2gVvdbGTEpvMJDbgs2t4");
    pub const ALPHA_Q: Pubkey = pubkey!("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA");
    pub const AQUIFER: Pubkey = pubkey!("AQU1FRd7papthgdrwPTTq5JacJh8YtwEXaBfKU3bTz45");
    pub const AXIOM_TRADE: Pubkey = pubkey!("FLASHX8DrLbgeR8FcfNV1F5krxYcYMUdBkrP1EPBtxB9");
    pub const BINANCE_WALLET_PROGRAM: Pubkey =
        pubkey!("B3111yJCeHBcA1bizdJjUFPALfhAfSRnAbJzGUtnt56A");
    pub const BISON_FI: Pubkey = pubkey!("BiSoNHVpsVZW2F7rx2eQ59yQwKxzU5NvBcmKshCSUypi");
    pub const BONK_SWAP: Pubkey = pubkey!("BSwp6bEBihVLdqJRKGgzjcGLHkcTuzmSo1TQkHepzH8p");
    pub const BYREAL: Pubkey = pubkey!("REALQqNEomY6cQGZJUGwywTBD2UmDT32rZcNnfxQ5N2");
    pub const DERIVERSE: Pubkey = pubkey!("DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD");
    pub const DFLOW_AGG: Pubkey = pubkey!("DF1ow4tspfHX9JwWJsAb9epbkA8hmpSEAtxXy1V27QBH");
    pub const DRIFT: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");
    pub const GOON_FI: Pubkey = pubkey!("goonERTdGsjnkZqWuVjs73BZ3Pb9qoCUdBUL17BnS5j");
    pub const GOON_FI_V2: Pubkey = pubkey!("goonuddtQRrWqqn5nFyczVKaie28f3kDkHWkHtURSLE");
    pub const GMGN: Pubkey = pubkey!("GMgnVFR8Jb39LoXsEVzb3DvBy3ywCmdmJquHUy1Lrkqb");
    pub const GMX: Pubkey = pubkey!("Gmso1uvJnLbawvw7yezdfCDcPydwW2s2iqG3w6MDucLo");
    pub const GOOSE_FX: Pubkey = pubkey!("GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT");
    pub const HEAVEN: Pubkey = pubkey!("HEAVENoP2qxoeuF8Dj2oT1GHEnu49U5mJYkdeC8BAX2o");
    pub const HUMIDI_FI: Pubkey = pubkey!("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp");
    pub const JUP_V6: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
    pub const LAUNCH_LAB: Pubkey = pubkey!("LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj");
    pub const LIFINITY_V2: Pubkey = pubkey!("2wT8Yq49kHgDzXuPxZSaeLaH1qbmGXtEyPy64bL7aD3c");
    pub const MANGO_V4: Pubkey = pubkey!("4MangoMjqJ2firMokCjjGgoK8d4MXcrgL7XJaL3w6fVg");
    pub const MANIFEST: Pubkey = pubkey!("MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms");
    pub const MERCURIAL_STABLE_SWAP: Pubkey =
        pubkey!("MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky");
    pub const METEORA_POOLS: Pubkey = pubkey!("Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB");
    pub const METEORA_DAMM_V2: Pubkey = pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
    pub const METEORA_DBC: Pubkey = pubkey!("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN");
    pub const METEORA_DLMM: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");
    pub const OBRIC_V2: Pubkey = pubkey!("obriQD1zbpyLz95G5n7nJe6a4DPjpFwa5XYPoNm113y");
    pub const OKX_ROUTER: Pubkey = pubkey!("6m2CDdhRgxpH4WjvdzxAYbGxwdGUz5MziiL5jek2kBma");
    pub const OKX_DEX_ROUTER_2: Pubkey = pubkey!("proVF4pMXVaYqmy4NjniPh4pqKNfMmsihgd4wdkCX3u");
    pub const ORCA: Pubkey = pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
    pub const PANCAKE_SWAP: Pubkey = pubkey!("HpNfyc2Saw7RKkQd8nEL4khUcuPhQ7WwY1B2qjx8jxFq");
    pub const PHOENIX: Pubkey = pubkey!("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY");
    pub const PUMP: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
    pub const PUMP_AMM: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
    pub const RAYDIUM_CLMM: Pubkey = pubkey!("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK");
    pub const RAYDIUM_CP_SWAP: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");
    pub const RAYDIUM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
    pub const SAROS_AMM: Pubkey = pubkey!("SSwapUtytfBdBn1b9NUGG6foMVPtcWgpRU32HToDUZr");
    pub const SAROS_DLMM: Pubkey = pubkey!("1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE");
    pub const SCORCH: Pubkey = pubkey!("SCoRcH8c2dpjvcJD6FiPbCSQyQgu3PcUAWj2Xxx3mqn");
    pub const SOL_FI: Pubkey = pubkey!("SoLFiHG9TfgtdUXUjWAxi3LtvYuFyDLVhBWxdMZxyCe");
    pub const SOL_FI_V2: Pubkey = pubkey!("SV2EYYJyRz2YhfXwXnhNAevDEui5Q6yrfyo13WtupPF");
    pub const STABBLE_STABLE: Pubkey = pubkey!("swapNyd8XiQwJ6ianp9snpu4brUqFxadzvHebnAXjJZ");
    pub const STABBLE_WEIGHTED_SWAP: Pubkey =
        pubkey!("swapFpHZwjELNnjvThjajtiVmkz3yPQEHjLtka2fwHW");
    pub const TESSERA_V: Pubkey = pubkey!("TessVdML9pBGgG9yGks7o4HewRaXVAMuoVj4x83GLQH");
    pub const TITAN_EXCHANGE_ROUTER: Pubkey =
        pubkey!("T1TANpTeScyeqVzzgNViGDNrkQ6qHz9KrSBS4aNXvGT");
    pub const ZERO_FI: Pubkey = pubkey!("ZERor4xhbUycZ6gb9ntrhqscUcZmAbQDjEAtCf4hbZY");
}
