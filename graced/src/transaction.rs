mod geyser;
pub mod instruction;
pub mod keys;

pub use instruction::SolanaInstruction;
use instruction::{IxWithStackHeight, StackIx};
use keys::AccountKeys;

use base64::{Engine, prelude::BASE64_STANDARD};
use yellowstone_grpc_proto::{geyser::SubscribeUpdateTransactionInfo, prelude as proto};
use solana_address::error::ParseAddressError;
use solana_bincode::limited_deserialize;
use solana_client::rpc_response::UiTransactionError;
use solana_message::{
    VersionedMessage,
    v0::{LoadedAddresses, MessageAddressTableLookup},
};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::{CompiledInstruction, versioned::TransactionVersion};
use solana_transaction_context::TransactionReturnData;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta,
    TransactionStatusMeta, TransactionTokenBalance, UiInstruction, UiLoadedAddresses,
    UiReturnDataEncoding, UiTransactionStatusMeta, UiTransactionTokenBalance,
};
use solana_transaction_status::{Reward, option_serializer::OptionSerializer};
use solana_vote_program::vote_instruction::VoteInstruction;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use thiserror::Error;

use crate::transaction::instruction::TransactionStack;

#[derive(Clone, Debug)]
pub struct SolanaTx {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub signature: Signature,
    pub is_vote: bool,
    pub version: Option<TransactionVersion>,
    pub message: Arc<VersionedMessage>,
    pub instructions: Vec<Arc<CompiledInstruction>>,
    pub account_keys: AccountKeys,
    pub meta: Arc<TxMetadata>,
}

impl SolanaTx {
    pub fn is_legacy(&self) -> bool {
        matches!(*self.message, VersionedMessage::Legacy(_))
    }

    pub fn is_versioned(&self) -> bool {
        matches!(*self.message, VersionedMessage::V0(_))
    }

    pub fn account_keys(&self) -> &AccountKeys {
        &self.account_keys
    }

    pub fn address_table_lookups(&self) -> Option<Vec<MessageAddressTableLookup>> {
        self.message.address_table_lookups().map(|x| x.to_vec())
    }

    pub fn signature(&self) -> Signature {
        self.signature
    }

    pub fn signers(&self) -> &[Pubkey] {
        let signatures = self.message.header().num_required_signatures as usize;
        &self.message.static_account_keys()[0..signatures]
    }

    pub fn build_stack(&self) -> TransactionStack {
        TransactionStack::build(self)
    }

    pub fn root_instructions(&self) -> impl Iterator<Item = StackIx> {
        self.instructions.iter().enumerate().map(|(idx, ix)| {
            let idx = idx as u8;
            let inner_ixs = self
                .meta
                .inner_instructions
                .get(&idx)
                .cloned()
                .unwrap_or_default();
            StackIx::build(ix.clone(), idx, &inner_ixs)
        })
    }
}

#[derive(Clone, Debug)]
pub struct TxMetadata {
    pub err: Option<UiTransactionError>,
    pub fee: u64,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub inner_instructions: HashMap<u8, Vec<IxWithStackHeight>>,
    pub log_messages: Vec<String>,
    pub pre_token_balances: Arc<Vec<TransactionTokenBalance>>,
    pub post_token_balances: Arc<Vec<TransactionTokenBalance>>,
    pub rewards: Vec<Reward>,
    pub loaded_addresses: Option<LoadedAddresses>,
    pub return_data: Option<TransactionReturnData>,
    pub compute_units_consumed: Option<u64>,
    pub cost_units: Option<u64>,
}

impl From<TransactionStatusMeta> for TxMetadata {
    fn from(value: TransactionStatusMeta) -> Self {
        Self {
            err: value.status.map_err(Into::into).err(),
            fee: value.fee,
            pre_balances: value.pre_balances,
            post_balances: value.post_balances,
            inner_instructions: value
                .inner_instructions
                .unwrap_or_default()
                .into_iter()
                .map(|ix| {
                    (
                        ix.index,
                        ix.instructions
                            .into_iter()
                            .map(|ix| {
                                IxWithStackHeight::new(Arc::new(ix.instruction), ix.stack_height)
                            })
                            .collect(),
                    )
                })
                .collect(),
            log_messages: value.log_messages.unwrap_or_default(),
            pre_token_balances: Arc::new(value.pre_token_balances.unwrap_or_default()),
            post_token_balances: Arc::new(value.post_token_balances.unwrap_or_default()),
            rewards: value.rewards.unwrap_or_default(),
            loaded_addresses: Some(value.loaded_addresses),
            return_data: value.return_data,
            compute_units_consumed: value.compute_units_consumed,
            cost_units: value.cost_units,
        }
    }
}

fn ui_instruction_to_compiled(
    ix: UiInstruction,
) -> Result<Option<(CompiledInstruction, Option<u32>)>, bs58::decode::Error> {
    let res = match ix {
        UiInstruction::Compiled(ix) => {
            let compiled_ix = CompiledInstruction {
                program_id_index: ix.program_id_index,
                accounts: ix.accounts,
                data: bs58::decode(&ix.data).into_vec()?,
            };
            (compiled_ix, ix.stack_height)
        }
        _ => return Ok(None),
    };

    Ok(Some(res))
}

fn serializer_to_default<T: Default>(t: OptionSerializer<T>) -> T {
    match t {
        OptionSerializer::Some(t) => t,
        _ => T::default(),
    }
}

fn serializer_to_option<T>(t: OptionSerializer<T>) -> Option<T> {
    match t {
        OptionSerializer::Some(t) => Some(t),
        _ => None,
    }
}

fn convert_ui_balances(balances: Vec<UiTransactionTokenBalance>) -> Vec<TransactionTokenBalance> {
    balances
        .into_iter()
        .map(|b| TransactionTokenBalance {
            account_index: b.account_index,
            mint: b.mint,
            ui_token_amount: b.ui_token_amount,
            owner: serializer_to_default(b.owner),
            program_id: serializer_to_default(b.program_id),
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum MetaConvertError {
    #[error("Failed converting pubkey: {0}")]
    InvalidPubkey(#[from] ParseAddressError),
    #[error("Ui instruction is not compiled")]
    MissingCompiledInstruction,
    #[error("Failed decoding base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Failed decoding base58: {0}")]
    Base58(#[from] bs58::decode::Error),
}

impl TryFrom<UiTransactionStatusMeta> for TxMetadata {
    type Error = MetaConvertError;

    fn try_from(value: UiTransactionStatusMeta) -> Result<Self, Self::Error> {
        let inner_ixs = match value.inner_instructions {
            OptionSerializer::Some(ixs) => ixs,
            _ => vec![],
        };

        let mut inner_instructions = HashMap::new();
        for ix in inner_ixs {
            let ixs = ix
                .instructions
                .into_iter()
                .map(|ix| {
                    let (ix, stack_height) = ui_instruction_to_compiled(ix)?
                        .ok_or(MetaConvertError::MissingCompiledInstruction)?;
                    let ix = IxWithStackHeight::new(Arc::new(ix), stack_height);
                    Ok::<_, MetaConvertError>(ix)
                })
                .collect::<Result<Vec<_>, _>>()?;
            inner_instructions.insert(ix.index, ixs);
        }

        let pre_token_balances = Arc::new(convert_ui_balances(serializer_to_default(
            value.pre_token_balances,
        )));
        let post_token_balances = Arc::new(convert_ui_balances(serializer_to_default(
            value.post_token_balances,
        )));

        let loaded_addresses = convert_ui_loaded_addresses(value.loaded_addresses.as_ref())?;

        let return_data = serializer_to_option(value.return_data)
            .map(|data| {
                let (encoded, encoding) = data.data;
                Ok::<_, MetaConvertError>(TransactionReturnData {
                    program_id: Pubkey::from_str(&data.program_id)?,
                    data: match encoding {
                        UiReturnDataEncoding::Base64 => BASE64_STANDARD.decode(&encoded)?,
                    },
                })
            })
            .transpose()?;

        Ok(Self {
            err: value.status.err(),
            fee: value.fee,
            pre_balances: value.pre_balances,
            post_balances: value.post_balances,
            inner_instructions,
            log_messages: serializer_to_default(value.log_messages),
            pre_token_balances,
            post_token_balances,
            rewards: serializer_to_default(value.rewards),
            loaded_addresses,
            return_data,
            compute_units_consumed: serializer_to_option(value.compute_units_consumed),
            cost_units: serializer_to_option(value.cost_units),
        })
    }
}

pub(super) fn convert_ui_loaded_addresses(
    loaded_addresses: OptionSerializer<&UiLoadedAddresses>,
) -> Result<Option<LoadedAddresses>, MetaConvertError> {
    Ok(serializer_to_option(loaded_addresses)
        .map(|addresses| {
            Ok::<_, <Pubkey as FromStr>::Err>(LoadedAddresses {
                writable: addresses
                    .writable
                    .iter()
                    .map(|key| Pubkey::from_str(key))
                    .collect::<Result<Vec<_>, _>>()?,
                readonly: addresses
                    .readonly
                    .iter()
                    .map(|key| Pubkey::from_str(key))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .transpose()?)
}

#[derive(Debug, Error)]
pub enum GrpcConvertError {
    #[error("No transaction present in GRPC update")]
    MissingTxUpdate,
    #[error("No transaction status meta present in GRPC update")]
    MissingTxStatusMeta,
    #[error("Error in conversion: {0}")]
    Other(String),
}

pub fn unified_tx_from_grpc(
    mut update: proto::SubscribeUpdateTransaction,
) -> Result<SolanaTx, GrpcConvertError> {
    let slot = update.slot;

    let SubscribeUpdateTransactionInfo {
        transaction,
        meta,
        is_vote,
        ..
    } = update
        .transaction
        .take()
        .ok_or(GrpcConvertError::MissingTxUpdate)?;

    let transaction = transaction.ok_or(GrpcConvertError::MissingTxUpdate)?;
    let meta = meta.ok_or(GrpcConvertError::MissingTxStatusMeta)?;

    let versioned_tx = geyser::create_tx_versioned(transaction)
        .map_err(|e| GrpcConvertError::Other(e.to_string()))?;
    let signature = *versioned_tx.signatures.first().expect("at least one");

    let instructions = versioned_tx
        .message
        .instructions()
        .to_vec()
        .into_iter()
        .map(Arc::new)
        .collect();

    let meta = geyser::create_tx_meta(meta).map_err(|e| GrpcConvertError::Other(e.to_string()))?;
    let meta = TxMetadata::from(meta);

    let account_keys = AccountKeys::new(
        versioned_tx.message.static_account_keys(),
        meta.loaded_addresses.as_ref(),
    );

    Ok(SolanaTx {
        signature,
        slot,
        is_vote,
        instructions,
        message: Arc::new(versioned_tx.message),
        meta: Arc::new(meta),
        block_time: None,
        account_keys,
        version: Some(TransactionVersion::Number(0)),
    })
}

fn is_vote_tx_present(message: &VersionedMessage) -> bool {
    message.instructions().iter().any(|i| {
        i.program_id(message.static_account_keys())
            .eq(&solana_vote_program::id())
            && limited_deserialize::<VoteInstruction>(
                &i.data,
                std::mem::size_of::<VoteInstruction>() as u64,
            )
            .map(|vi| vi.is_simple_vote())
            .unwrap_or(false)
    })
}

#[derive(Debug, Error)]
pub enum RpcConvertError {
    #[error("Failed to decode RPC encoded transaction")]
    FailedDecode,
    #[error("No transaction status meta present in RPC Tx")]
    MissingTxStatusMeta,
    #[error("Failed to convert transaction meta: {0}")]
    MetaConvert(#[from] MetaConvertError),
}

pub fn unified_tx_from_rpc(
    rpc_tx: EncodedTransactionWithStatusMeta,
    slot: u64,
    block_time: Option<i64>,
) -> Result<SolanaTx, RpcConvertError> {
    let meta = rpc_tx.meta.ok_or(RpcConvertError::MissingTxStatusMeta)?;

    let meta = TxMetadata::try_from(meta)?;

    let versioned_tx = rpc_tx
        .transaction
        .decode()
        .ok_or(RpcConvertError::FailedDecode)?;
    let is_vote = is_vote_tx_present(&versioned_tx.message);

    let instructions = versioned_tx
        .message
        .instructions()
        .to_vec()
        .into_iter()
        .map(Arc::new)
        .collect();

    let signature = *versioned_tx.signatures.first().expect("at least one");

    let account_keys = AccountKeys::new(
        versioned_tx.message.static_account_keys(),
        meta.loaded_addresses.as_ref(),
    );

    Ok(SolanaTx {
        signature,
        slot,
        block_time,
        is_vote,
        instructions,
        message: Arc::new(versioned_tx.message),
        meta: Arc::new(meta),
        account_keys,
        version: rpc_tx.version,
    })
}

impl TryFrom<EncodedConfirmedTransactionWithStatusMeta> for SolanaTx {
    type Error = RpcConvertError;

    fn try_from(value: EncodedConfirmedTransactionWithStatusMeta) -> Result<Self, Self::Error> {
        unified_tx_from_rpc(value.transaction, value.slot, value.block_time)
    }
}

impl TryFrom<proto::SubscribeUpdateTransaction> for SolanaTx {
    type Error = GrpcConvertError;

    fn try_from(value: proto::SubscribeUpdateTransaction) -> Result<Self, Self::Error> {
        unified_tx_from_grpc(value)
    }
}
