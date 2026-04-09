use crate::fee_estimation::DEFAULT_SIGNATURE_FEE;
use anchor_lang::AccountDeserialize;
use solana_commitment_config::CommitmentConfig;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_message::Message;
use solana_message::VersionedMessage;
use solana_pubkey::Pubkey;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client::rpc_client::SerializableTransaction;
use solana_rpc_client_api::config::{
    RpcSimulateTransactionAccountsConfig, RpcSimulateTransactionConfig,
};
use solana_rpc_client_api::response::{Response, RpcSimulateTransactionResult};
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction::Transaction;

pub const TX_ACTION_SIMULATION: u8 = 0;
pub const TX_ACTION_SENT_TX: u8 = 1;
pub const TX_ACTION_ESTIMATE_FEE: u8 = 2;

fn parse_compute_budget_instruction(data: &[u8]) -> Option<ComputeBudgetInstruction> {
    let discriminator = *data.first()?;

    match discriminator {
        2 => {
            let units = u32::from_le_bytes(data.get(1..5)?.try_into().ok()?);
            Some(ComputeBudgetInstruction::SetComputeUnitLimit(units))
        }
        3 => {
            let micro_lamports = u64::from_le_bytes(data.get(1..9)?.try_into().ok()?);
            Some(ComputeBudgetInstruction::SetComputeUnitPrice(
                micro_lamports,
            ))
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct TransactionPayload {
    pub wallet_memo: String,
    pub payload: Vec<u8>,
}
#[derive(serde::Deserialize, serde::Serialize)]
pub struct SimulationResult {
    pub result: Response<RpcSimulateTransactionResult>,
    pub pre_payer_balance: u64,
    pub post_payer_balance: u64,
}

#[derive(Debug, Clone)]
pub struct RpcArgs {
    pub rpc_url: String,
    pub priority_fee: u64,
    pub tx_action: u8,
    pub keypair_path: Option<String>,
}

impl RpcArgs {
    pub fn rpc_client(&self) -> RpcClient {
        RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::finalized())
    }

    pub fn get_account<T: AccountDeserialize>(&self, address: Pubkey) -> anyhow::Result<T> {
        let account = self.rpc_client().get_account(&address)?;
        let mut data = account.data.as_slice();
        match T::try_deserialize(&mut data) {
            Ok(value) => Ok(value),
            Err(_) => {
                let mut data = account.data.as_slice();
                T::try_deserialize_unchecked(&mut data).map_err(Into::into)
            }
        }
    }

    pub fn send_transaction_wrapper(
        &self,
        tx: &Transaction,
        max_retries: u64,
        payer: Pubkey,
        wallet_memo: String,
        sucess_cb: fn(wallet_memo: String, sig: Signature),
        failed_cb: fn(wallet_memo: String),
    ) -> anyhow::Result<TransactionPayload> {
        match self.tx_action {
            TX_ACTION_SIMULATION => {
                let client =
                    RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::finalized());
                let pre_payer_balance = client.get_account(&payer)?.lamports;
                let mut result = self.simulation_transaction(vec![payer.to_string()], tx)?;
                let post_payer_balance = if result.value.err.is_none() {
                    let accounts = result.value.accounts.unwrap();
                    let account = accounts[0].clone().unwrap();
                    account.lamports
                } else {
                    pre_payer_balance
                };

                println!("{} {:?}", wallet_memo, result.value.logs);

                result.value.accounts = None;
                Ok(TransactionPayload {
                    wallet_memo,
                    payload: bincode::serialize(&SimulationResult {
                        result,
                        pre_payer_balance,
                        post_payer_balance,
                    })?,
                })
            }
            TX_ACTION_ESTIMATE_FEE => {
                let fee = self.estimate_transaction_fee(wallet_memo.clone(), tx)?;
                Ok(TransactionPayload {
                    wallet_memo,
                    payload: bincode::serialize(&fee)?,
                })
            }
            TX_ACTION_SENT_TX => match self.send_transaction(tx, max_retries) {
                Ok(sig) => {
                    sucess_cb(wallet_memo.clone(), sig);
                    Ok(TransactionPayload {
                        wallet_memo,
                        payload: bincode::serialize(&sig)?,
                    })
                }
                Err(e) => {
                    failed_cb(wallet_memo);
                    Err(anyhow::Error::msg(e))
                }
            },
            _ => {
                panic!("Tx action {} is not supported", self.tx_action);
            }
        }
    }

    pub fn send_versioned_transaction_wrapper(
        &self,
        tx: &VersionedTransaction,
        max_retries: u64,
        payer: Pubkey,
        wallet_memo: String,
        sucess_cb: fn(wallet_memo: String, sig: Signature),
        failed_cb: fn(wallet_memo: String),
    ) -> anyhow::Result<TransactionPayload> {
        match self.tx_action {
            TX_ACTION_SIMULATION => {
                let client =
                    RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::finalized());
                let pre_payer_balance = client.get_account(&payer)?.lamports;
                let mut result = self.simulation_transaction(vec![payer.to_string()], tx)?;
                let post_payer_balance = if result.value.err.is_none() {
                    let accounts = result.value.accounts.unwrap();
                    let account = accounts[0].clone().unwrap();
                    account.lamports
                } else {
                    pre_payer_balance
                };
                result.value.accounts = None;
                Ok(TransactionPayload {
                    wallet_memo,
                    payload: bincode::serialize(&SimulationResult {
                        result,
                        pre_payer_balance,
                        post_payer_balance,
                    })?,
                })
            }
            TX_ACTION_ESTIMATE_FEE => {
                let fee = self.estimate_versioned_transaction_fee(wallet_memo.clone(), tx)?;
                Ok(TransactionPayload {
                    wallet_memo,
                    payload: bincode::serialize(&fee)?,
                })
            }
            TX_ACTION_SENT_TX => match self.send_transaction(tx, max_retries) {
                Ok(sig) => {
                    sucess_cb(wallet_memo.clone(), sig);
                    Ok(TransactionPayload {
                        wallet_memo,
                        payload: bincode::serialize(&sig)?,
                    })
                }
                Err(e) => {
                    failed_cb(wallet_memo);
                    Err(anyhow::Error::msg(e))
                }
            },
            _ => panic!("Tx action {} is not supported", self.tx_action),
        }
    }

    pub fn send_transaction(
        &self,
        tx: &impl SerializableTransaction,
        max_retries: u64,
    ) -> anyhow::Result<Signature> {
        let client = RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::finalized());
        for _i in 0..max_retries {
            match client.send_and_confirm_transaction_with_spinner(tx) {
                Ok(_) => {
                    return Ok(*tx.get_signature());
                }
                Err(e) => {
                    println!("cannot send tx {:?}", e);
                }
            }
        }
        return Err(anyhow::Error::msg("Cannot send transaction"));
    }

    pub fn simulation_transaction(
        &self,
        addresses: Vec<String>,
        tx: &impl SerializableTransaction,
    ) -> anyhow::Result<Response<RpcSimulateTransactionResult>> {
        // get pre balance
        let client = RpcClient::new_with_commitment(&self.rpc_url, CommitmentConfig::finalized());
        let result = client.simulate_transaction_with_config(
            tx,
            RpcSimulateTransactionConfig {
                commitment: Some(CommitmentConfig::finalized()),
                accounts: Some(RpcSimulateTransactionAccountsConfig {
                    addresses,
                    encoding: None,
                }),
                ..RpcSimulateTransactionConfig::default()
            },
        )?;
        Ok(result)
    }

    pub fn estimate_transaction_fee(
        &self,
        wallet_memo: String,
        tx: &Transaction,
    ) -> anyhow::Result<u64> {
        let num_signature = tx.signatures.len();
        let base_fee = (num_signature as u64) * DEFAULT_SIGNATURE_FEE;

        let x = match parse_compute_budget_instruction(&tx.message.instructions[0].data).unwrap() {
            ComputeBudgetInstruction::SetComputeUnitPrice(price) => price,
            ComputeBudgetInstruction::SetComputeUnitLimit(compute_unit) => compute_unit as u64,
            _ => 0,
        };

        let y = match parse_compute_budget_instruction(&tx.message.instructions[1].data).unwrap() {
            ComputeBudgetInstruction::SetComputeUnitPrice(price) => price,
            ComputeBudgetInstruction::SetComputeUnitLimit(compute_unit) => compute_unit as u64,
            _ => 0,
        };

        if x == 0 && y == 0 {
            println!("Cannot estimate price {}", wallet_memo);
            return Ok(0);
        }
        let total_fee = base_fee + x * y;
        // println!("Wallet {} fee {}", wallet_memo, total_fee);
        Ok(total_fee)
    }

    pub fn estimate_versioned_transaction_fee(
        &self,
        wallet_memo: String,
        tx: &VersionedTransaction,
    ) -> anyhow::Result<u64> {
        let num_signature = tx.signatures.len();
        let base_fee = (num_signature as u64) * DEFAULT_SIGNATURE_FEE;

        let instructions = match tx.message.clone() {
            VersionedMessage::Legacy(Message {
                header: _,
                account_keys: _,
                recent_blockhash: _,
                instructions,
            }) => instructions,
            VersionedMessage::V0(solana_message::v0::Message {
                header: _,
                account_keys: _,
                recent_blockhash: _,
                instructions,
                address_table_lookups: _,
            }) => instructions,
        };
        let x = match parse_compute_budget_instruction(&instructions[1].data).unwrap() {
            ComputeBudgetInstruction::SetComputeUnitPrice(price) => price,
            ComputeBudgetInstruction::SetComputeUnitLimit(compute_unit) => compute_unit as u64,
            _ => 0,
        };

        let y = match parse_compute_budget_instruction(&instructions[0].data).unwrap() {
            ComputeBudgetInstruction::SetComputeUnitPrice(price) => price,
            ComputeBudgetInstruction::SetComputeUnitLimit(compute_unit) => compute_unit as u64,
            _ => 0,
        };

        if x == 0 && y == 0 {
            println!("Cannot estimate price {}", wallet_memo);
            return Ok(0);
        }
        // println!("{} {}", x, y);
        let total_fee = base_fee + x * y;
        println!("Wallet {} fee {}", wallet_memo, total_fee);
        Ok(total_fee)
    }
}
