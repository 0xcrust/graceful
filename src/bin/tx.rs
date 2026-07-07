use std::str::FromStr;

use graceful::transaction::SolanaTx;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{CommitmentConfig, RpcTransactionConfig},
};
use solana_signature::Signature;
use solana_transaction_status::UiTransactionEncoding;

// const TX: &str = "5USDT1XsuHoPUPaQBcrfUVh6FeSzPRFV8AsCQHa9PVh58Vt7i8swfMdfgsjPhBCMtRv34aQST5JuLekQajMdtznK";
// const TX: &str = "46GPGhky2xbBipHzXQasspJW8253fvgaZbvCE2su87vwDM4FB9S6N9NPEmH23tLh6gzGar2isowXFentKfJA59xc";
// const TX: &str = "2MC3eSGkjeTXgzRpWUcE1oTJbsAAXEMG93RfuFjKGApJnSLYteTLu1nM9y38KvCnHSZtTV7Bu4UaAbKrYU4HykK5";

const TXS: &[(&str, usize)] = &[
    (
        "ARDT7JHUbD7DywhP7FjhNQQMF8fhR5qWsbuC66eJzL1hZ1GXvdVgas6hmdL7avEDDeuyAFxzfofwGf8zXbd6NHm",
        2,
    ),
    (
        "5USDT1XsuHoPUPaQBcrfUVh6FeSzPRFV8AsCQHa9PVh58Vt7i8swfMdfgsjPhBCMtRv34aQST5JuLekQajMdtznK",
        2,
    ),
    (
        "46GPGhky2xbBipHzXQasspJW8253fvgaZbvCE2su87vwDM4FB9S6N9NPEmH23tLh6gzGar2isowXFentKfJA59xc",
        2,
    ),
    (
        "2MC3eSGkjeTXgzRpWUcE1oTJbsAAXEMG93RfuFjKGApJnSLYteTLu1nM9y38KvCnHSZtTV7Bu4UaAbKrYU4HykK5",
        2,
    ),
    (
        "4mPmGQrwvDFV3ZJn3jGhyVxUxCPBsjqrwyFev9xNCsc7JUwhB5CCG2gYH1DpddF2aNp9k8vPWrWBF5q5KPVxQu8a",
        2,
    ),
    (
        "6KhUtsffiBMs3vgHC2yFKVwcVsJDz66xrEzUzmyRzar7Tw6p3EiB11K9tghdWsoLhDwwhMHJmCZR8JfxDF4ociq",
        2,
    ),
    (
        "C6AUhCXqQhARvQ4Tt834HQhT2qqngacCQYSdujAeENDEXQVYW8FPsELM861vYmepgbnRhcvQ7FXP4W5KLYdPci3",
        2,
    ),
    (
        "4ZywEhUzUFnF7WXziAbt6HwqT2cW33wWauFFC5Ap6iUqNg4imeeG1PUGjAjQGkniCt6x9tLmQmQW7mebcdqfuVA8",
        2,
    ),
    (
        "XfyH3qAnBRcyDPtHYWhfTJEdqLdBHq9cPxEXAb2Ve2G2NXvQBUPJhnPaF1og2zweg3nt8bYDmtqpi6rrUEojbx1",
        2,
    ),
    (
        "4ZSXPV4KrMw8FUdSPJ2ywwjXteLe6xALZ4n2oZeN1S46C91LfJKtM4s6RqTKsWewFdTgS3cuaUHzM8WwhZMCpZcP",
        2,
    ),
    (
        "4rhE43Le2DzH3cYUb6R7AGhFgLthy2zgNE67RWnCW8nx9m8uHXRe9HVTu2w3YS1DsULiqF7mdqWNp93KijqHR5jw",
        2,
    ),
    (
        "21w48geQzDZvypHaeAFRsK2ZHs1YAnjx6nt1mLMMU9N6p6nJoHQjByLkzrvSx94cM16ZAinHzA8QVYEVi6ukXJTU",
        3,
    ),
    (
        "3p2DxopSFZZgWFoeKbec8V9y1C8xPRZzCDjAJVTjvGXrrpx7DuihWo9S3QkvaSUH9KCwbj7dTku3DYLS6N7ncQTa",
        2,
    ),
    (
        "5qBdCRWtMH6p4jFwftrNsJPRf9JY1MD6C9FUfsA7KpkRKb1rXem665UWSjT9XYbuRFSZwbjPuiFSaMfAg24xiSqi",
        3,
    ),
];

#[tokio::main]
pub async fn main() {
    let rpc = RpcClient::new(
        "https://mainnet.helius-rpc.com/?api-key=2de78024-3b5a-4969-a952-e155a0df98d9".to_string(),
    );

    for (tx, idx) in TXS {
        let tx = rpc
            .get_transaction_with_config(
                &Signature::from_str(tx).unwrap(),
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )
            .await
            .unwrap();

        let tx = SolanaTx::try_from(tx).unwrap();

        let ix = &tx.instructions[*idx];

        let data = &ix.data[0..8];
        let program_id = tx.account_keys().get(ix.program_id_index).unwrap();
        println!("program: {}. data: {:?}", program_id, data);
    }
}

// [42, 2, 140, 146, 5, 0, 0, 0]
// [42, 2, 224, 4, 0, 0, 0, 0]
// [42, 2, 61, 10, 0, 0, 0, 0]
