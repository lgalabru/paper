mod ui;
mod events_observer;

use crate::types::{ChainConfig, MainConfig};
use crate::utils;

use std::collections::HashMap;
use std::fs::{File, self};
use std::hash::Hash;
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::task;
use std::sync::mpsc::{Receiver, Sender, channel};
use bollard::Docker;
use bollard::container::{Config, KillContainerOptions, CreateContainerOptions, StartContainerOptions, LogsOptions};
use bollard::models::{HostConfig, PortBinding};
use bollard::network::{ConnectNetworkOptions, CreateNetworkOptions, PruneNetworksOptions};
use bollard::image::CreateImageOptions;
use deno_core::futures::TryStreamExt;

use events_observer::start_events_observer;

pub fn run_devnet(devnet: &mut DevnetOrchestrator) {
    match block_on(do_run_devnet(devnet)) {
        Err(_e) => std::process::exit(1),
        _ => {}
    };
}

pub fn block_on<F, R>(future: F) -> R
where
    F: std::future::Future<Output = R>,
{
    let rt = utils::create_basic_runtime();
    rt.block_on(future)
}

pub async fn do_run_devnet(
    devnet: &mut DevnetOrchestrator,
) -> Result<bool, String> {


    let event_tx = devnet.event_tx.clone().unwrap();
    let (termination_success_tx, termination_success_rx) = channel();
    devnet.termination_success_tx = Some(termination_success_tx);

    let terminator = event_tx.clone();

    ctrlc::set_handler(move || {
        terminator.send(DevnetEvent::Terminate)
            .expect("Unable to terminate devnet");
        // let _res = termination_success_rx.recv();
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");

    let orchestrator_port = match &devnet.network_config {
        Some(ref network_config) => match network_config.devnet {
            Some(ref devnet_config) => Ok(devnet_config.orchestrator_port),
            _ => Err("Unable to retrieve Devnet config")
        }
        _ => Err("Unable to retrieve Devnet config")
    }?;

    // let join_handle = std::thread::spawn(move || {
    //     let future = start_events_observer(orchestrator_port);
    //     let rt = utils::create_basic_runtime();
    //     rt.block_on(future);
    // });

    // devnet.start().await;

    let event_tx_simulator = event_tx.clone();

    let join_handle = std::thread::spawn(move || {
        let mut i = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            event_tx_simulator.send(DevnetEvent::Log(LogData {
                level: LogLevel::Info,
                message: "Hello world".into(),
                occured_at: 0
            })).unwrap();
            event_tx_simulator.send(DevnetEvent::Block(BlockData {
                block_height: i,
                bitcoin_block_height: i,
                block_hash: format!("{}", i),
                bitcoin_block_hash: format!("{}", i),
                transactions: vec![
                    Transaction {
                        txid: "".to_string(),
                        success: i % 2 == 0,
                        result: format!("(ok u1)"),
                        events: vec![],
                    },
                    Transaction {
                        txid: "".to_string(),
                        success: (i + 1) % 2 == 0,
                        result: format!("(err u3)"),
                        events: vec![],
                    },
                    Transaction {
                        txid: "".to_string(),
                        success: (i + 2) % 2 == 0,
                        result: format!("(ok err)"),
                        events: vec![],
                    },
                ]
            })).unwrap();
            i += 1;
        }
    });

    let event_rx = devnet.event_rx
        .take()
        .expect("Unable to get event receiver");

    ui::start_ui(event_tx, event_rx);

    join_handle.join().unwrap();

    Ok(true)
}

pub enum DevnetEvent {
    Log(LogData),
    KeyEvent(crossterm::event::KeyEvent),
    Tick,
    Restart,
    Terminate,
    ServiceStatus(ServiceStatusData),
    Block(BlockData),
    Microblock(MicroblockData),
    MempoolAdmission(MempoolAdmissionData),
}

pub enum LogLevel {
    Error,
    Warning,
    Info,
    Success,
}

pub struct LogData {
    pub occured_at: u32,
    pub message: String,
    pub level: LogLevel,
}

pub struct ServiceStatusData {
    pub order: u8,
    pub status: u8,
    pub name: String,
    pub comment: String,
}

#[derive(Clone)]
pub struct Transaction {
    pub txid: String,
    pub success: bool,
    pub result: String,
    pub events: Vec<String>,
}

pub struct Event {
    pub content: String,
}

#[derive(Clone)]
pub struct BlockData {
    pub block_height: u32,
    pub block_hash: String,
    pub bitcoin_block_height: u32,
    pub bitcoin_block_hash: String,
    pub transactions: Vec<Transaction>
}

pub struct MicroblockData {
    pub seq: u32,
    pub transactions: Vec<Transaction>
}

pub struct MempoolAdmissionData {
    pub txid: String,
}

#[derive(Default, Debug)]
pub struct DevnetOrchestrator {
    name: String,
    network_name: String,
    manifest_path: PathBuf,
    network_config: Option<ChainConfig>,
    event_rx: Option<Receiver<DevnetEvent>>,
    pub event_tx: Option<Sender<DevnetEvent>>,
    termination_success_tx: Option<Sender<bool>>,
    stacks_blockchain_container_id: Option<String>,
    stacks_blockchain_api_container_id: Option<String>,
    stacks_explorer_container_id: Option<String>,
    bitcoin_blockchain_container_id: Option<String>,
    bitcoin_explorer_container_id: Option<String>,
    postgres_container_id: Option<String>,
    docker_client: Option<Docker>,
}

impl DevnetOrchestrator {

    pub fn new(manifest_path: PathBuf) -> DevnetOrchestrator {
        let docker_client = Docker::connect_with_socket_defaults().unwrap();

        let mut project_path = manifest_path.clone();
        project_path.pop();
    
        let mut network_config_path = project_path.clone();
        network_config_path.push("settings");
        network_config_path.push("Development.toml");
    
        let network_config = ChainConfig::from_path(&network_config_path);
        let project_config = MainConfig::from_path(&manifest_path);
        let name = project_config.project.name.clone();
        let network_name = format!("{}.devnet", name);

        let (event_tx, event_rx) = channel();

        DevnetOrchestrator {
            name,
            network_name,
            manifest_path,
            network_config: Some(network_config),
            event_rx: Some(event_rx),
            event_tx: Some(event_tx),
            docker_client: Some(docker_client),
            ..Default::default()
        }
    }

    pub async fn start(&mut self) {
        let (docker, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, devnet_config),
                _ => return
            }
            _ => return
        };

        fs::create_dir(format!("{}", devnet_config.working_dir)).expect("Unable to create working dir");
        fs::create_dir(format!("{}/conf", devnet_config.working_dir)).expect("Unable to create working dir");
        fs::create_dir(format!("{}/data", devnet_config.working_dir)).expect("Unable to create working dir");
        fs::create_dir(format!("{}/data/bitcoin", devnet_config.working_dir)).expect("Unable to create working dir");
        fs::create_dir(format!("{}/data/stacks", devnet_config.working_dir)).expect("Unable to create working dir");

        let _network = docker.create_network(CreateNetworkOptions {
            name: self.network_name.clone(),
            driver: "bridge".to_string(),
            ..Default::default()
        }).await.expect("Unable to create network");

        // Start bitcoind
        match self.boot_bitcoin_container().await {
            Ok(_) => {},
            Err(message) => {
                println!("{}", message);
                self.terminate().await;
                std::process::exit(1);
            }
        };

        // Start postgres
        match self.boot_postgres_container().await {
            Ok(_) => {},
            Err(message) => {
                println!("{}", message);
                self.terminate().await;
                std::process::exit(1);
            }
        };

        // Start stacks-blockchain-api
        match self.boot_stacks_blockchain_api_container().await {
            Ok(_) => {},
            Err(message) => {
                println!("{}", message);
                self.terminate().await;
                std::process::exit(1);
            }
        };

        // Start stacks-blockchain
        match self.boot_stacks_blockchain_container().await {
            Ok(_) => {},
            Err(message) => {
                println!("{}", message);
                self.terminate().await;
                std::process::exit(1);
            }
        };

        // Start stacks-explorer
        match self.boot_stacks_explorer_container().await {
            Ok(_) => {},
            Err(message) => {
                println!("{}", message);
                self.terminate().await;
                std::process::exit(1);
            }
        };

        // Start local observer
        // TODO
    }

    // if working_dir empty:
    //      -> write config files
    // else
    //      -> read config files

    pub async fn boot_bitcoin_container(&mut self) -> Result<(), String> {
        let (docker, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, devnet_config),
                _ => return Err("Unable to get devnet configuration".into())
            }
            _ => return Err("Unable to get Docker client".into())
        };
    
        let _info = docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: devnet_config.bitcoind_image_url.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| "Unable to create image".to_string())?;
    
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", devnet_config.bitcoin_controller_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.bitcoin_controller_port)),
            }]),
        );
        port_bindings.insert(
            format!("{}/tcp", devnet_config.bitcoind_p2p_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.bitcoind_p2p_port)),
            }]),
        );
        port_bindings.insert(
            format!("{}/tcp", devnet_config.bitcoind_rpc_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.bitcoind_rpc_port)),
            }]),
        );

        let bitcoind_conf = format!(r#"
server=1
regtest=1
rpcallowip=0.0.0.0/0
rpcallowip=::/0
rpcuser={}
rpcpassword={}
txindex=1
listen=1
rpcserialversion=0
rpcworkqueue=100

[regtest]
bind=0.0.0.0:{}
rpcbind=0.0.0.0:{}
rpcport={}
"#,
            devnet_config.bitcoind_username,
            devnet_config.bitcoind_password,
            devnet_config.bitcoind_p2p_port,
            devnet_config.bitcoind_rpc_port,
            devnet_config.bitcoind_rpc_port,
        );
        let mut bitcoind_conf_path = PathBuf::from(&devnet_config.working_dir);
        bitcoind_conf_path.push("conf/bitcoin.conf");
        let mut file = File::create(bitcoind_conf_path).expect("Unable to create bitcoind.conf");
        file.write_all(bitcoind_conf.as_bytes()).expect("Unable to write bitcoind.conf");

        let bitcoin_controller_conf = format!(r#"
[network]
rpc_bind = "0.0.0.0:{}"
block_time = {}
miner_address = "{}"
faucet_address = "{}"
bitcoind_rpc_host = "0.0.0.0:{}"
bitcoind_rpc_user = "{}"
bitcoind_rpc_pass = "{}"
genesis_timestamp = 1588615296
whitelisted_rpc_calls = [
    "listunspent",
    "listwallets",
    "createwallet",
    "importaddress",
    "sendrawtransaction",
    "getrawtransaction",
    "scantxoutset",
    "getrawmempool",
    "getblockhash",
]
[[blocks]]
count = 1
block_time = 10000
ignore_txs = false
"#, 
            devnet_config.bitcoin_controller_port,
            devnet_config.bitcoin_controller_block_time,
            devnet_config.miner_btc_address,
            devnet_config.miner_btc_address,
            devnet_config.bitcoind_rpc_port,
            devnet_config.bitcoind_username,
            devnet_config.bitcoind_password,
        );
        let mut bitcoin_controller_conf_path = PathBuf::from(&devnet_config.working_dir);
        bitcoin_controller_conf_path.push("conf/puppet-chain.toml");
    
        let mut file = File::create(bitcoin_controller_conf_path).expect("Unable to create bitcoin_controller.toml");
        file.write_all(bitcoin_controller_conf.as_bytes()).expect("Unable to create bitcoin_controller.toml");
    

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", devnet_config.bitcoin_controller_port), HashMap::new());
        exposed_ports.insert(format!("{}/tcp", devnet_config.bitcoind_rpc_port), HashMap::new());
        exposed_ports.insert(format!("{}/tcp", devnet_config.bitcoind_p2p_port), HashMap::new());

        let config = Config {
            image: Some(devnet_config.bitcoind_image_url.clone()),
            domainname: Some(self.network_name.to_string()),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            entrypoint: Some(vec![]),
            env: Some(vec!["DYNAMIC_GENESIS_TIMESTAMP=1".to_string()]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                
                binds: Some(vec![
                    format!("{}/conf:/etc/bitcoin", devnet_config.working_dir),
                    format!("{}/data/bitcoin:/root/.bitcoin", devnet_config.working_dir),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let options = CreateContainerOptions {
            name: format!("bitcoin.{}", self.network_name)
        };

        let container = docker
            .create_container::<String, String>(Some(options), config)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?
            .id;
        
        self.bitcoin_blockchain_container_id = Some(container.clone());

        docker.start_container::<String>(&container, None)
            .await
            .map_err(|_| "Unable to start container".to_string())?;
        
        let res = docker.connect_network(&self.network_name, ConnectNetworkOptions {
            container,
            ..Default::default()
        }).await;

        if let Err(e) = res {
            let err = format!("Error connecting container: {}", e);
            println!("{}", err);
            return Err(err)
        }

        Ok(())
    }

    pub async fn boot_stacks_blockchain_container(&mut self) -> Result<(), String> {
        let (docker, network_config, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, network_config, devnet_config),
                _ => return Err("Unable to get devnet configuration".into())
            }
            _ => return Err("Unable to get Docker client".into())
        };
    
        let _info = docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: devnet_config.stacks_node_image_url.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| "Unable to create image".to_string())?;
    
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", devnet_config.stacks_node_p2p_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.stacks_node_p2p_port)),
            }]),
        );
        port_bindings.insert(
            format!("{}/tcp", devnet_config.stacks_node_rpc_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.stacks_node_rpc_port)),
            }]),
        );

        let mut stacks_conf = format!(r#"
[node]
working_dir = "/devnet"
rpc_bind = "0.0.0.0:{}"
p2p_bind = "0.0.0.0:{}"
miner = true
seed = "{}"
local_peer_seed = "{}"
wait_time_for_microblocks = 1000

[[events_observer]]
endpoint = "{}"
retry_count = 255
events_keys = ["*"]

[burnchain]
chain = "bitcoin"
mode = "krypton"
peer_host = "{}"
username = "{}"
password = "{}"
rpc_port = {}
peer_port = {}

[[events_observer]]
endpoint = "host.docker.internal:{}"
retry_count = 255
events_keys = ["*"]
"#,
            devnet_config.stacks_node_rpc_port,
            devnet_config.stacks_node_p2p_port,
            devnet_config.miner_secret_key_hex,
            devnet_config.miner_secret_key_hex,
            format!("stacks-api.{}:{}", self.network_name, devnet_config.stacks_api_events_port),
            format!("bitcoin.{}", self.network_name),
            devnet_config.bitcoind_username,
            devnet_config.bitcoind_password,
            devnet_config.bitcoin_controller_port,
            devnet_config.bitcoind_p2p_port,
            devnet_config.orchestrator_port,
        );

        for (_, account) in network_config.accounts.iter() {
            stacks_conf.push_str(&format!(r#"
[[ustx_balance]]
address = "{}"
amount = {}
"#,
                account.address,
                account.balance
            ));
        }
        
        for events_observer in devnet_config.stacks_node_events_observers.iter() {
            stacks_conf.push_str(&format!(r#"
[[events_observer]]
endpoint = "{}"
retry_count = 255
events_keys = ["*"]
"#,
                events_observer,
            ));
        }

        let mut stacks_conf_path = PathBuf::from(&devnet_config.working_dir);
        stacks_conf_path.push("conf/Config.toml");
        let mut file = File::create(stacks_conf_path).expect("Unable to create bitcoind.conf");
        file.write_all(stacks_conf.as_bytes()).expect("Unable to write bitcoind.conf");

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", devnet_config.stacks_node_rpc_port), HashMap::new());
        exposed_ports.insert(format!("{}/tcp", devnet_config.stacks_node_p2p_port), HashMap::new());

        let config = Config {
            image: Some(devnet_config.stacks_node_image_url.clone()),
            domainname: Some(self.network_name.to_string()),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            entrypoint: Some(vec!["stacks-node".into(), "start".into(), "--config=/src/stacks-node/Config.toml".into()]),
            env: Some(vec![
                "STACKS_LOG_PP=1".to_string(),
                "BLOCKSTACK_USE_TEST_GENESIS_CHAINSTATE=1".to_string(),
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                
                binds: Some(vec![
                    format!("{}/conf:/src/stacks-node/", devnet_config.working_dir),
                    format!("{}/data/stacks:/devnet/", devnet_config.working_dir),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let options = CreateContainerOptions {
            name: format!("stacks.{}", self.network_name)
        };

        let container = docker
            .create_container::<String, String>(Some(options), config)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?
            .id;
        
        self.stacks_blockchain_container_id = Some(container.clone());

        docker.start_container::<String>(&container, None)
            .await
            .map_err(|_| "Unable to start container".to_string())?;
        
        let res = docker.connect_network(&self.network_name, ConnectNetworkOptions {
            container,
            ..Default::default()
        }).await;

        if let Err(e) = res {
            let err = format!("Error connecting container: {}", e);
            println!("{}", err);
            return Err(err)
        }

        Ok(())
    }

    pub async fn boot_stacks_blockchain_api_container(&mut self) -> Result<(), String> {
        let (docker, _, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, network_config, devnet_config),
                _ => return Err("Unable to get devnet configuration".into())
            }
            _ => return Err("Unable to get Docker client".into())
        };
    
        let _info = docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: devnet_config.stacks_api_image_url.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| "Unable to create image".to_string())?;
    
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", devnet_config.stacks_api_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.stacks_api_port)),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", devnet_config.stacks_api_port), HashMap::new());

        let config = Config {
            image: Some(devnet_config.stacks_api_image_url.clone()),
            domainname: Some(self.network_name.to_string()),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            env: Some(vec![
                format!("STACKS_CORE_RPC_HOST=stacks.{}", self.network_name),
                format!("STACKS_BLOCKCHAIN_API_DB=pg"),
                format!("STACKS_BLOCKCHAIN_API_PORT={}", devnet_config.stacks_api_port),
                format!("STACKS_BLOCKCHAIN_API_HOST=0.0.0.0"),
                format!("STACKS_CORE_EVENT_PORT={}", devnet_config.stacks_api_events_port),
                format!("STACKS_CORE_EVENT_HOST=0.0.0.0"),
                format!("PG_HOST=postgres.{}", self.network_name),
                format!("PG_PORT={}", devnet_config.postgres_port),
                format!("PG_USER={}", devnet_config.postgres_username),
                format!("PG_PASSWORD={}", devnet_config.postgres_password),
                format!("PG_DATABASE={}", devnet_config.postgres_database),
                format!("STACKS_CHAIN_ID=2147483648"),
                format!("V2_POX_MIN_AMOUNT_USTX=90000000260"),
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),                
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let options = CreateContainerOptions {
            name: format!("stacks-api.{}", self.network_name)
        };

        let container = docker
            .create_container::<String, String>(Some(options), config)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?
            .id;
        
        self.stacks_blockchain_api_container_id = Some(container.clone());

        docker.start_container::<String>(&container, None)
            .await
            .map_err(|_| "Unable to start container".to_string())?;
        
        let res = docker.connect_network(&self.network_name, ConnectNetworkOptions {
            container,
            ..Default::default()
        }).await;

        if let Err(e) = res {
            let err = format!("Error connecting container: {}", e);
            println!("{}", err);
            return Err(err)
        }

        Ok(())
    }

    pub async fn boot_postgres_container(&mut self) -> Result<(), String> {
        let (docker, _, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, network_config, devnet_config),
                _ => return Err("Unable to get devnet configuration".into())
            }
            _ => return Err("Unable to get Docker client".into())
        };
    
        let _info = docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: devnet_config.postgres_image_url.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|_| "Unable to create image".to_string())?;
    
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", devnet_config.postgres_port),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.postgres_port)),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", devnet_config.postgres_port), HashMap::new());

        let config = Config {
            image: Some(devnet_config.postgres_image_url.clone()),
            domainname: Some(self.network_name.to_string()),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            env: Some(vec![
                format!("POSTGRES_PASSWORD={}", devnet_config.postgres_password),
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),                
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let options = CreateContainerOptions {
            name: format!("postgres.{}", self.network_name)
        };

        let container = docker
            .create_container::<String, String>(Some(options), config)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?
            .id;
        
        self.postgres_container_id = Some(container.clone());

        docker.start_container::<String>(&container, None)
            .await
            .map_err(|_| "Unable to start container".to_string())?;
        
        let res = docker.connect_network(&self.network_name, ConnectNetworkOptions {
            container,
            ..Default::default()
        }).await;

        if let Err(e) = res {
            let err = format!("Error connecting container: {}", e);
            println!("{}", err);
            return Err(err)
        }

        Ok(())
    }

    pub async fn boot_stacks_explorer_container(&mut self) -> Result<(), String> {
        let (docker, _, devnet_config) = match (&self.docker_client, &self.network_config) {
            (Some(ref docker), Some(ref network_config)) => match network_config.devnet {
                Some(ref devnet_config) => (docker, network_config, devnet_config),
                _ => return Err("Unable to get devnet configuration".into())
            }
            _ => return Err("Unable to get Docker client".into())
        };
    
        let _info = docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: devnet_config.stacks_explorer_image_url.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| format!("Unable to create image: {}", e))?;
    
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", 3000),
            Some(vec![PortBinding {
                host_ip: Some(String::from("0.0.0.0")),
                host_port: Some(format!("{}/tcp", devnet_config.stacks_explorer_port)),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(format!("{}/tcp", 3000), HashMap::new());

        let config = Config {
            image: Some(devnet_config.stacks_explorer_image_url.clone()),
            domainname: Some(self.network_name.to_string()),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            env: Some(vec![
                format!("NEXT_PUBLIC_MAINNET_API_SERVER=http://stacks-api.{}:{}", self.network_name, devnet_config.stacks_api_port),
                format!("NEXT_PUBLIC_TESTNET_API_SERVER=http://stacks-api.{}:{}", self.network_name, devnet_config.stacks_api_port),
                format!("MOCKNET_API_SERVER=http://stacks-api.{}:{}", self.network_name, devnet_config.stacks_api_port),
                format!("TESTNET_API_SERVER=http://stacks-api.{}:{}", self.network_name, devnet_config.stacks_api_port),
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),                
                ..Default::default()
            }),
            ..Default::default()
        };
        
        let options = CreateContainerOptions {
            name: format!("stacks-explorer.{}", self.network_name)
        };

        let container = docker
            .create_container::<String, String>(Some(options), config)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?
            .id;
        
        self.stacks_explorer_container_id = Some(container.clone());

        docker.start_container::<String>(&container, None)
            .await
            .map_err(|e| format!("Unable to create container: {}", e))?;
        
        let res = docker.connect_network(&self.network_name, ConnectNetworkOptions {
            container,
            ..Default::default()
        }).await;

        if let Err(e) = res {
            let err = format!("Error connecting container: {}", e);
            println!("{}", err);
            return Err(err)
        }

        Ok(())
    }

    pub async fn restart(&mut self) {

    }

    pub async fn terminate(&mut self) {
        let docker = match self.docker_client {
            Some(ref docker) => docker,
            None => std::process::exit(1)
        };

        println!("Initiating termination sequence");

        let options = Some(KillContainerOptions{
            signal: "SIGKILL",
        });        

        // Terminate containers
        if let Some(ref bitcoin_explorer_container_id) = self.bitcoin_explorer_container_id {
            println!("Terminating bitcoin_explorer");
            let _ = docker.kill_container(bitcoin_explorer_container_id, options.clone()).await;
            let _ = docker.remove_container(bitcoin_explorer_container_id, None);
        }

        if let Some(ref stacks_explorer_container_id) = self.stacks_explorer_container_id {
            println!("Terminating stacks_explorer");
            let _ = docker.kill_container(stacks_explorer_container_id, options.clone()).await;
        }

        if let Some(ref bitcoin_blockchain_container_id) = self.bitcoin_blockchain_container_id {
            println!("Terminating bitcoin_blockchain");
            let _ = docker.kill_container(bitcoin_blockchain_container_id, options.clone()).await;
            let _ = docker.remove_container(bitcoin_blockchain_container_id, None);
        }

        if let Some(ref stacks_blockchain_api_container_id) = self.stacks_blockchain_api_container_id {
            println!("Terminating stacks_blockchain_api");
            let _ = docker.kill_container(stacks_blockchain_api_container_id, options.clone()).await;
        }

        if let Some(ref postgres_container_id) = self.postgres_container_id {
            println!("Terminating postgres");
            let _ = docker.kill_container(postgres_container_id, options.clone()).await;
        }

        if let Some(ref stacks_blockchain_container_id) = self.stacks_blockchain_container_id {
            println!("Terminating stacks_blockchain");
            let _ = docker.kill_container(stacks_blockchain_container_id, options).await;
        }

        // Prune network
        println!("Pruning network {}", self.network_name);
        let mut filters = HashMap::new();
        filters.insert("label".to_string(), vec![format!("label={}", self.network_name)]);
        docker.prune_networks(Some(PruneNetworksOptions { filters })).await;

        let _ = docker.remove_network(&self.network_name).await;

        println!("Ended termination sequence");
        if let Some(ref tx) = self.termination_success_tx {
            tx.send(true).expect("Unable to confirm termination");
        }
    }
}