use super::DevnetEvent;
use crate::integrate::{BlockData, MempoolAdmissionData, ServiceStatusData, Status, Transaction};
use crate::publish::{publish_contracts, Network};
use crate::types::{self, AccountConfig, DevnetConfig};
use crate::utils::stacks::{transactions, StacksRpc};
use clarity_repl::clarity::representations::ClarityName;
use clarity_repl::clarity::types::{BuffData, SequenceData, TupleData, Value as ClarityValue};
use clarity_repl::clarity::util::address::AddressHashMode;
use clarity_repl::clarity::util::hash::{hex_bytes, Hash160};
use rocket::config::{Config, Environment, LoggingLevel};
use rocket::State;
use rocket_contrib::json::Json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::error::Error;
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct NewBurnBlock {
    burn_block_hash: String,
    burn_block_height: u32,
    reward_slot_holders: Vec<String>,
    burn_amount: u32,
}

#[derive(Deserialize)]
pub struct NewBlock {
    block_height: u32,
    block_hash: String,
    burn_block_height: u32,
    burn_block_hash: String,
    transactions: Vec<NewTransaction>,
    // reward_slot_holders: Vec<String>,
    // burn_amount: u32,
}

#[derive(Deserialize)]
pub struct NewTransaction {
    txid: String,
    status: String,
    raw_result: String,
    // tx_index: u32,
}

#[derive(Clone, Debug)]
pub struct EventObserverConfig {
    pub devnet_config: DevnetConfig,
    pub accounts: BTreeMap<String, AccountConfig>,
    pub manifest_path: PathBuf,
    pub pox_info: PoxInfo,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PoxInfo {
    contract_id: String,
    pox_activation_threshold_ustx: u64,
    first_burnchain_block_height: u32,
    prepare_phase_block_length: u32,
    reward_phase_block_length: u32,
    reward_slots: u32,
    total_liquid_supply_ustx: u64,
    next_cycle: PoxCycle,
}

impl PoxInfo {
    pub fn default() -> PoxInfo {
        PoxInfo {
            contract_id: "ST000000000000000000002AMW42H.pox".into(),
            pox_activation_threshold_ustx: 0,
            first_burnchain_block_height: 100,
            prepare_phase_block_length: 1,
            reward_phase_block_length: 4,
            reward_slots: 8,
            total_liquid_supply_ustx: 1000000000000000,
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PoxCycle {
    min_threshold_ustx: u64,
}

pub async fn start_events_observer(
    events_config: EventObserverConfig,
    devnet_event_tx: Sender<DevnetEvent>,
    terminator_rx: Receiver<bool>,
) -> Result<(), Box<dyn Error>> {
    let port = events_config.devnet_config.orchestrator_port;

    let config = Config::build(Environment::Production)
        .address("127.0.0.1")
        .port(port)
        .log_level(LoggingLevel::Off)
        .finalize()?;

    rocket::custom(config)
        .manage(RwLock::new(events_config))
        .manage(Arc::new(Mutex::new(devnet_event_tx.clone())))
        .mount(
            "/",
            routes![
                handle_new_burn_block,
                handle_new_block,
                handle_new_microblocks,
                handle_new_mempool_tx,
                handle_drop_mempool_tx
            ],
        )
        .launch();

    match terminator_rx.recv() {
        Ok(true) => {
            devnet_event_tx
                .send(DevnetEvent::info("Terminating event observer".into()))
                .expect("Unable to terminate event observer");
        }
        _ => {}
    }
    Ok(())
}

#[post(
    "/new_burn_block",
    format = "application/json",
    data = "<new_burn_block>"
)]
pub fn handle_new_burn_block(
    _config: State<RwLock<EventObserverConfig>>,
    devnet_events_tx: State<Arc<Mutex<Sender<DevnetEvent>>>>,
    new_burn_block: Json<NewBurnBlock>,
) -> Json<Value> {
    let devnet_events_tx = devnet_events_tx.inner();

    match devnet_events_tx.lock() {
        Ok(tx) => {
            let _ = tx.send(DevnetEvent::debug(format!(
                "Bitcoin block #{} received",
                new_burn_block.burn_block_height
            )));
            let _ = tx.send(DevnetEvent::ServiceStatus(ServiceStatusData {
                order: 0,
                status: Status::Green,
                name: "bitcoin-node".into(),
                comment: format!(
                    "mining blocks (chaintip = #{})",
                    new_burn_block.burn_block_height
                ),
            }));
        }
        _ => {}
    };

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/new_block", format = "application/json", data = "<new_block>")]
pub fn handle_new_block(
    config: State<RwLock<EventObserverConfig>>,
    devnet_events_tx: State<Arc<Mutex<Sender<DevnetEvent>>>>,
    new_block: Json<NewBlock>,
) -> Json<Value> {
    let devnet_events_tx = devnet_events_tx.inner();
    let config = config.inner();

    if let Ok(tx) = devnet_events_tx.lock() {
        let _ = tx.send(DevnetEvent::ServiceStatus(ServiceStatusData {
            order: 1,
            status: Status::Green,
            name: "stacks-node".into(),
            comment: format!("mining blocks (chaintip = #{})", new_block.block_height),
        }));
        let _ = tx.send(DevnetEvent::info(format!(
            "Block #{} anchored in Bitcoin block #{} includes {} transactions",
            new_block.block_height,
            new_block.burn_block_height,
            new_block.transactions.len(),
        )));

        if new_block.block_height == 1 {
            // We just received the Stacks Genesis block.
            // With that, we will be:
            // - Publishing the contracts
            if let Ok(config_reader) = config.read() {
                let logs = match publish_contracts(
                    config_reader.manifest_path.clone(),
                    Network::Devnet,
                ) {
                    Ok(res) => res.iter().map(|l| DevnetEvent::success(l.into())).collect(),
                    Err(e) => vec![DevnetEvent::error(e.into())],
                };
                for log in logs.into_iter() {
                    let _ = tx.send(log);
                }
            }
        }

        let updated_config = if let Ok(config_reader) = config.read() {
            let mut updated_config = config_reader.clone();
            let url = format!(
                "http://0.0.0.0:{}/v2/pox",
                updated_config.devnet_config.stacks_node_rpc_port
            );
            if let Ok(reponse) = reqwest::blocking::get(url) {
                if let Ok(pox_info) = reponse.json() {
                    updated_config.pox_info = pox_info;
                    Some(updated_config)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(updated_config) = updated_config {
            if let Ok(mut config_writer) = config.write() {
                *config_writer = updated_config;
            }
        }


        if let Ok(config_reader) = config.read() {
            let pox_cycle_length = config_reader.pox_info.prepare_phase_block_length
                + config_reader.pox_info.reward_phase_block_length;
            let pox_cycle_id = (new_block.burn_block_height
                - config_reader.pox_info.first_burnchain_block_height)
                / pox_cycle_length;
            let _ = tx.send(DevnetEvent::Block(BlockData {
                block_height: new_block.block_height,
                block_hash: new_block.block_hash.clone(),
                bitcoin_block_height: new_block.burn_block_height,
                bitcoin_block_hash: new_block.burn_block_hash.clone(),
                first_burnchain_block_height: config_reader.pox_info.first_burnchain_block_height,
                pox_cycle_length,
                pox_cycle_id,
                transactions: new_block
                    .transactions
                    .iter()
                    .map(|t| Transaction {
                        txid: t.txid.clone(),
                        success: t.status == "success",
                        result: t.raw_result.clone(),
                        events: vec![],
                    })
                    .collect(),
            }));

            // Every penultimate block, we check if some stacking orders should be submitted before the next
            // cycle starts.
            if new_block.burn_block_height % pox_cycle_length == (pox_cycle_length - 2) {
                for pox_stacking_order in config_reader.devnet_config.pox_stacking_orders.iter() {
                    if pox_stacking_order.start_at_cycle == (pox_cycle_id + 1) {
                        let account = match config_reader.accounts.get(&pox_stacking_order.wallet) {
                            None => continue,
                            Some(account) => account,
                        };
                        let url = format!(
                            "http://0.0.0.0:{}",
                            config_reader.devnet_config.stacks_node_rpc_port
                        );
                        let stacks_rpc = StacksRpc::new(url);
                        let default_fee = 1000;
                        let nonce = stacks_rpc
                            .get_nonce(account.address.to_string())
                            .expect("Unable to retrieve nonce");

                        let stx_amount = config_reader.pox_info.next_cycle.min_threshold_ustx
                            * pox_stacking_order.slots;
                        let (_, _, account_secret_keu) = types::compute_addresses(
                            &account.mnemonic,
                            &account.derivation,
                            account.is_mainnet,
                        );
                        let addr_bytes = Hash160([0u8; 20]);
                        let addr_version = AddressHashMode::SerializeP2PKH;
                        let stack_stx_tx = transactions::build_contrat_call_transaction(
                            config_reader.pox_info.contract_id.clone(),
                            "stack-stx".into(),
                            vec![
                                ClarityValue::UInt(stx_amount.into()),
                                ClarityValue::Tuple(
                                    TupleData::from_data(vec![
                                        (
                                            ClarityName::try_from("version".to_owned()).unwrap(),
                                            ClarityValue::buff_from_byte(addr_version as u8),
                                        ),
                                        (
                                            ClarityName::try_from("hashbytes".to_owned()).unwrap(),
                                            ClarityValue::Sequence(SequenceData::Buffer(
                                                BuffData {
                                                    data: addr_bytes.as_bytes().to_vec(),
                                                },
                                            )),
                                        ),
                                    ])
                                    .unwrap(),
                                ),
                                ClarityValue::UInt((new_block.burn_block_height - 1).into()),
                                ClarityValue::UInt(pox_stacking_order.duration.into()),
                            ],
                            nonce,
                            default_fee,
                            &hex_bytes(&account_secret_keu).unwrap(),
                        );
                        let _ = stacks_rpc
                            .post_transaction(stack_stx_tx)
                            .expect("Unable to broadcast transaction");
                    }
                }
            }
        }
    };

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/new_microblocks", format = "application/json")]
pub fn handle_new_microblocks(
    _devnet_events_tx: State<Arc<Mutex<Sender<DevnetEvent>>>>,
) -> Json<Value> {
    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/new_mempool_tx", format = "application/json", data = "<raw_txs>")]
pub fn handle_new_mempool_tx(
    _config: State<RwLock<EventObserverConfig>>,
    devnet_events_tx: State<Arc<Mutex<Sender<DevnetEvent>>>>,
    raw_txs: Json<Vec<String>>,
) -> Json<Value> {
    if let Ok(tx) = devnet_events_tx.lock() {
        for raw_tx in raw_txs.iter() {
            let _ = tx.send(DevnetEvent::MempoolAdmission(MempoolAdmissionData {
                txid: raw_tx.to_string(),
            }));
        }
    }

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/drop_mempool_tx", format = "application/json")]
pub fn handle_drop_mempool_tx() -> Json<Value> {
    println!("POST /drop_mempool_tx");

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}
