use {
    anchor_lang::AccountDeserialize,
    anchor_spl::{metadata::MetadataAccount, token_2022::spl_token_2022},
    log::{debug, error, warn},
    solana_client::nonblocking::rpc_client::RpcClient,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_token_2022_interface::{
        extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
        state::Mint as BaseMint,
    },
    spl_token_interface::state::Mint,
    spl_token_metadata_interface::{borsh::BorshDeserialize, state::TokenMetadata},
};

pub async fn fetch_mint_infos(
    rpc_client: &RpcClient,
    mints: &[Pubkey],
) -> anyhow::Result<Vec<Option<RawMintInfo>>> {
    let mint_accounts = rpc_client.get_multiple_accounts(mints).await?;
    let metadatas = mints
        .iter()
        .map(|mint| find_metadata_pda(mint).0)
        .collect::<Vec<_>>();
    let metadata_accounts = rpc_client.get_multiple_accounts(&metadatas).await?;

    let mut final_results = Vec::with_capacity(mints.len());
    let mut no_metadata = vec![];
    for ((mint_account, metadata_account), mint) in mint_accounts
        .into_iter()
        .zip(metadata_accounts)
        .zip(mints)
    {
        let Some(mint_account) = mint_account else {
            log::error!("No mint account found for mint {}", mint);
            final_results.push(None);
            continue;
        };

        let mint_state = match Mint::unpack(&mint_account.data[..Mint::LEN]) {
            Ok(mint) => mint,
            Err(e) => {
                log::error!(
                    "Failed to deserialize mint account for mint {}: {}",
                    mint,
                    e
                );
                final_results.push(None);
                continue;
            }
        };

        if let Some(account) = metadata_account {
            let (name, symbol, uri) = match MetadataAccount::try_deserialize(&mut &account.data[..])
            {
                Ok(metadata) => (
                    Some(metadata.name.trim_end_matches('\u{0000}').to_string()),
                    Some(metadata.symbol.trim_end_matches('\u{0000}').to_string()),
                    Some(metadata.uri.trim_end_matches('\u{0000}').to_string()),
                ),
                Err(e) => {
                    log::error!(
                        "Failed to deserialize metadata account for mint {}: {}",
                        mint,
                        e
                    );
                    (None, None, None)
                }
            };

            final_results.push(Some(RawMintInfo {
                pubkey: *mint,
                decimals: mint_state.decimals,
                supply: mint_state.supply,
                program_id: mint_account.owner,
                token_2022: mint_account.owner == spl_token_2022::ID,
                mint_authority: mint_state.mint_authority.into(),
                freeze_authority: mint_state.freeze_authority.into(),
                name,
                symbol,
                uri,
            }))
        } else {
            debug!("No metadata account found for mint {}", mint);
            let idx = final_results.len();
            final_results.push(None);
            no_metadata.push((mint_account, mint_state, *mint, idx));
        };
    }

    for (mint_account, mint_state, mint, idx) in no_metadata {
        let mut info = RawMintInfo {
            pubkey: mint,
            decimals: mint_state.decimals,
            supply: mint_state.supply,
            program_id: mint_account.owner,
            token_2022: mint_account.owner == spl_token_2022::ID,
            mint_authority: mint_state.mint_authority.into(),
            freeze_authority: mint_state.freeze_authority.into(),
            name: None,
            symbol: None,
            uri: None,
        };

        if mint_account.owner == spl_token_2022::ID {
            let state = StateWithExtensions::<'_, BaseMint>::unpack(&mint_account.data)?;
            let extensions = state.get_extension_types()?;

            if extensions.contains(&ExtensionType::TokenMetadata) {
                let mut x = state.get_extension_bytes::<TokenMetadata>()?;
                let meta = TokenMetadata::deserialize(&mut x)?;

                info.name = Some(meta.name);
                info.symbol = Some(meta.symbol);
                info.uri = Some(meta.uri);
            } else {
                error!(
                    "no metadata found for token-22 mint. extensions: {:?}",
                    extensions
                );
            }
        }

        if info.name.is_none() {
            warn!("Failed to get metadata for mint {}", mint);
        }

        debug_assert!(final_results[idx].is_none());
        final_results[idx] = Some(info);
    }

    Ok(final_results)
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RawMintInfo {
    pub pubkey: Pubkey,
    pub decimals: u8,
    pub supply: u64,
    pub program_id: Pubkey,
    pub token_2022: bool,
    pub mint_authority: Option<Pubkey>,
    pub freeze_authority: Option<Pubkey>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub uri: Option<String>,
}

fn find_metadata_pda(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "metadata".as_bytes(),
            anchor_spl::metadata::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::metadata::ID,
    )
}
