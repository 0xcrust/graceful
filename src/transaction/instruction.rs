use std::{borrow::Cow, sync::Arc};

use solana_transaction::CompiledInstruction;

use crate::transaction::SolanaTx;

pub trait SolanaInstruction
where
    Self: Sized,
{
    fn accounts(&self) -> Arc<Vec<u8>>;
    fn data(&self) -> Result<Cow<'_, [u8]>, Box<dyn std::error::Error>>;
    fn program_id(&self) -> u8;
    fn inner_ixs(&self) -> impl Iterator<Item = &Self>;
}

pub struct TransactionStack {
    pub ixs: Vec<StackIx>,
}

impl TransactionStack {
    pub fn build(tx: &SolanaTx) -> Self {
        Self {
            ixs: tx.instructions().collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StackIx {
    pub ix: Arc<CompiledInstruction>,
    pub inner: Vec<StackIx>,
    pub path: Vec<u8>,
}

impl SolanaInstruction for StackIx {
    fn accounts(&self) -> Arc<Vec<u8>> {
        Arc::new(self.ix.accounts.clone())
    }

    fn data(&self) -> Result<Cow<'_, [u8]>, Box<dyn std::error::Error>> {
        Ok(Cow::Borrowed(&self.ix.data))
    }

    fn program_id(&self) -> u8 {
        self.ix.program_id_index
    }

    fn inner_ixs(&self) -> impl Iterator<Item = &Self> {
        self.inner.iter()
    }
}

impl StackIx {
    pub fn build(ix: Arc<CompiledInstruction>, idx: u8, inner_ixs: &[IxWithStackHeight]) -> Self {
        build_ix_stack(ix, inner_ixs, idx)
    }
}

#[derive(Clone, Debug)]
pub struct IxWithStackHeight {
    pub ix: Arc<CompiledInstruction>,
    pub height: Option<u32>,
}

impl IxWithStackHeight {
    pub(crate) fn new(ix: Arc<CompiledInstruction>, height: Option<u32>) -> Self {
        Self { ix, height }
    }
}

pub fn build_ix_stack(
    ix: Arc<CompiledInstruction>,
    inner_ixs: &[IxWithStackHeight],
    index: u8,
) -> StackIx {
    // Initialize the root node that will hold the top-level inner instructions
    let root_path = vec![index];

    // Stack stores (Node, Height, NextChildIndex)
    // NextChildIndex is used to generate the path for the next child added to this node
    let mut stack: Vec<(StackIx, Option<u32>, u8)> = Vec::new();

    // We need to track children added to the root separately if we want to avoid
    // putting the root in the stack initially (since root height is implicit)
    let mut root_inner: Vec<StackIx> = Vec::new();
    let mut root_child_count = 0;

    for ix in inner_ixs {
        let instruction = &ix.ix;
        let height = ix.height;
        // 1. Close scopes
        // Pop from stack until we find a valid parent for the current instruction
        loop {
            let is_valid_parent = stack.last().is_some_and(|(_, p_h, _)| {
                match (p_h, height) {
                    (Some(p), Some(c)) => c > *p,
                    _ => false, // None acts as a barrier; cannot be parent or child
                }
            });

            if is_valid_parent {
                break;
            }

            if let Some((finished_node, _, _)) = stack.pop() {
                // Append the finished node to its parent (new stack top) or root
                if let Some((parent_node, _, count)) = stack.last_mut() {
                    parent_node.inner.push(finished_node);
                    *count += 1;
                } else {
                    root_inner.push(finished_node);
                    root_child_count += 1;
                }
            } else {
                break;
            }
        }

        // 2. Determine Path for the new node
        let path = if let Some((parent_node, _, count)) = stack.last() {
            // Path is parent_path + current_child_index
            let mut path = parent_node.path.clone();
            path.push(*count);
            path
        } else {
            // No parent in stack, so it's a child of the root
            let mut path = root_path.clone();
            path.push(root_child_count);
            path
        };

        // 3. Create and push new node
        let node = StackIx {
            ix: instruction.clone(),
            inner: vec![],
            path,
        };

        stack.push((node, height, 0));
    }

    // 4. Drain remaining items in the stack
    while let Some((finished_node, _, _)) = stack.pop() {
        if let Some((parent_node, _, count)) = stack.last_mut() {
            parent_node.inner.push(finished_node);
            *count += 1;
        } else {
            root_inner.push(finished_node);
        }
    }

    StackIx {
        ix,
        inner: root_inner,
        path: root_path,
    }
}

#[cfg(test)]
pub mod instruction_stack_tests {
    use crate::transaction::{SolanaTx, instruction::TransactionStack};
    use std::str::FromStr;

    use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
    use solana_pubkey::{Pubkey, pubkey};
    use solana_sdk_ids::{compute_budget, system_program};
    use solana_signature::Signature;
    use solana_transaction_status::UiTransactionEncoding;

    const ATOKEN: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
    const DFLOW_V4: Pubkey = pubkey!("DF1ow4tspfHX9JwWJsAb9epbkA8hmpSEAtxXy1V27QBH");
    const METEORA_DLMM: Pubkey = pubkey!("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo");

    #[tokio::test]
    async fn sample_transaction() {
        let rpc_url = if dotenv::dotenv().is_ok() {
            std::env::var("RPC_URL").unwrap_or("https://api.mainnet-beta.solana.com".to_string())
        } else {
            "https://api.mainnet-beta.solana.com".to_string()
        };
        let rpc = RpcClient::new(rpc_url);

        let tx = rpc.get_transaction_with_config(
            &Signature::from_str("VxSahqMfCGXAi3tMcg6vV9qX8c1LpPPcLeJGuNMg8W4pf7hJaSubsS1hC77uK1Z1RfLef165mGKeCrLpuzWBVaX").unwrap(),
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Base64),
                max_supported_transaction_version: Some(0),
                ..Default::default()
            }
        ).await.unwrap();

        let tx = SolanaTx::try_from(tx).unwrap();
        let stack = TransactionStack::build(&tx);

        let account_keys = tx.account_keys();

        assert_eq!(stack.ixs.len(), 5);

        let cu1 = &stack.ixs[0];
        let cu2 = &stack.ixs[1];
        let dflow_swap = &stack.ixs[2];
        let dflow_unwrap_sol = &stack.ixs[3];
        let system_transfer = &stack.ixs[4];

        assert_eq!(
            *account_keys.get(cu1.ix.program_id_index as usize).unwrap(),
            compute_budget::ID
        );
        assert!(cu1.inner.is_empty());

        assert_eq!(
            *account_keys.get(cu2.ix.program_id_index as usize).unwrap(),
            compute_budget::ID
        );
        assert!(cu2.inner.is_empty());

        assert_eq!(
            *account_keys
                .get(dflow_swap.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );
        let ata_create = &dflow_swap.inner[0];
        assert_eq!(
            *account_keys
                .get(ata_create.ix.program_id_index as usize)
                .unwrap(),
            ATOKEN
        );
        let met_dlmm_swap = &dflow_swap.inner[1];
        assert_eq!(
            *account_keys
                .get(met_dlmm_swap.ix.program_id_index as usize)
                .unwrap(),
            METEORA_DLMM
        );
        assert_eq!(met_dlmm_swap.inner.len(), 3); // transfer, transfer, cpi-event
        for (i, inner) in met_dlmm_swap.inner.iter().take(2).enumerate() {
            if i != met_dlmm_swap.inner.len() - 1 {
                assert_eq!(
                    *account_keys
                        .get(inner.ix.program_id_index as usize)
                        .unwrap(),
                    spl_token_interface::ID
                );
            } else {
                assert_eq!(
                    *account_keys
                        .get(met_dlmm_swap.inner[2].ix.program_id_index as usize)
                        .unwrap(),
                    METEORA_DLMM
                );
            }
            assert!(inner.inner.is_empty());
        }

        let dlmm_event_cpi = &dflow_swap.inner[2];
        assert_eq!(
            *account_keys
                .get(dlmm_event_cpi.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );

        let humidifi_swap = &dflow_swap.inner[3];
        assert_eq!(humidifi_swap.inner.len(), 2);
        for inner in humidifi_swap.inner.iter() {
            assert_eq!(
                *account_keys
                    .get(inner.ix.program_id_index as usize)
                    .unwrap(),
                spl_token_interface::ID
            );
            assert!(inner.inner.is_empty());
        }

        let humidifi_event_cpi = &dflow_swap.inner[4];
        assert_eq!(
            *account_keys
                .get(humidifi_event_cpi.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );

        let tessera_swap = &dflow_swap.inner[5];
        for inner in tessera_swap.inner.iter() {
            assert_eq!(
                *account_keys
                    .get(inner.ix.program_id_index as usize)
                    .unwrap(),
                spl_token_interface::ID
            );
            assert!(inner.inner.is_empty());
        }

        let tessera_event_cpi = &dflow_swap.inner[6];
        assert_eq!(
            *account_keys
                .get(tessera_event_cpi.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );

        /* idxs 7 & 8 are tokenProgram CloseAccount & AssociatedTokenProgram Create */

        let tessera_swap2 = &dflow_swap.inner[9];
        for inner in tessera_swap2.inner.iter() {
            assert_eq!(
                *account_keys
                    .get(inner.ix.program_id_index as usize)
                    .unwrap(),
                spl_token_interface::ID
            );
            assert!(inner.inner.is_empty());
        }

        let tessera_event_cpi2 = &dflow_swap.inner[10];
        assert_eq!(
            *account_keys
                .get(tessera_event_cpi2.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );

        assert_eq!(
            *account_keys
                .get(dflow_unwrap_sol.ix.program_id_index as usize)
                .unwrap(),
            DFLOW_V4
        );
        assert_eq!(
            *account_keys
                .get(system_transfer.ix.program_id_index as usize)
                .unwrap(),
            system_program::ID
        );
    }
}
