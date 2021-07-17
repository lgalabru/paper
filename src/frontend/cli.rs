use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{prelude::*, BufReader, Read};
use std::path::PathBuf;
use std::{env, process};

use crate::console::load_session;
use crate::test::run_tests;
use crate::types::{MainConfig, MainConfigFile, RequirementConfig};
use crate::{
    generators::{
        self,
        changes::{Changes, TOMLEdition},
    },
    utils::mnemonic,
};

use clarity_repl::clarity::codec::transaction::{
    StacksTransaction, StacksTransactionSigner, TransactionAnchorMode, TransactionAuth,
    TransactionPayload, TransactionPostConditionMode, TransactionPublicKeyEncoding,
    TransactionSmartContract, TransactionSpendingCondition,
};
use clarity_repl::clarity::codec::StacksMessageCodec;
use clarity_repl::{
    clarity::{
        codec::{
            transaction::{
                RecoverableSignature, SinglesigHashMode, SinglesigSpendingCondition,
                TransactionVersion,
            },
            StacksString,
        },
        util::{
            address::AddressHashMode,
            secp256k1::{Secp256k1PrivateKey, Secp256k1PublicKey},
            StacksAddress,
        },
    },
    repl,
};

use clap::Clap;
use libsecp256k1::{PublicKey, SecretKey};
use tiny_hderive::bip32::ExtendedPrivKey;
use toml;

#[derive(Clap)]
#[clap(version = option_env!("CARGO_PKG_VERSION").expect("Unable to detect version"))]
struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap)]
enum Command {
    /// New subcommand
    #[clap(name = "new")]
    New(GenerateProject),
    /// Contract subcommand
    #[clap(name = "contract")]
    Contract(Contract),
    /// Console subcommand
    #[clap(name = "console")]
    Console(Console),
    /// Test subcommand
    #[clap(name = "test")]
    Test(Test),
    /// Check subcommand
    #[clap(name = "check")]
    Check(Check),
    /// Deploy subcommand
    #[clap(name = "deploy")]
    Deploy(Deploy),
    /// Run subcommand
    #[clap(name = "run")]
    Run(Run),
}

#[derive(Clap)]
enum Contract {
    /// New contract subcommand
    #[clap(name = "new")]
    NewContract(NewContract),
    /// Import contract subcommand
    #[clap(name = "requirement")]
    LinkContract(LinkContract),
    /// Fork contract subcommand
    #[clap(name = "fork")]
    ForkContract(ForkContract),
}

#[derive(Clap)]
struct GenerateProject {
    /// Project's name
    pub name: String,
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
}

#[derive(Clap)]
struct NewContract {
    /// Contract's name
    pub name: String,
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
}

#[derive(Clap)]
struct LinkContract {
    /// Contract id
    pub contract_id: String,
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
}

#[derive(Clap, Debug)]
struct ForkContract {
    /// Contract id
    pub contract_id: String,
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
    // /// Fork contract and all its dependencies
    // #[clap(short = 'r')]
    // pub recursive: bool,
}

#[derive(Clap)]
struct Console {
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
}

#[derive(Clap)]
struct Test {
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Generate coverage
    #[clap(long = "coverage")]
    pub coverage: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
    /// Relaunch tests on updates
    #[clap(long = "watch")]
    pub watch: bool,
    /// Files to includes
    pub files: Vec<String>,
}

#[derive(Clap)]
struct Run {
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Script to run
    pub script: String,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
    /// Allow access to wallets
    #[clap(long = "allow-wallets")]
    pub allow_wallets: bool,
}

#[derive(Clap)]
struct Deploy {
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Deploy contracts on mocknet, using settings/Mocknet.toml
    #[clap(long = "mocknet", conflicts_with = "testnet")]
    pub mocknet: bool,
    /// Deploy contracts on mocknet, using settings/Testnet.toml
    #[clap(long = "testnet", conflicts_with = "mocknet")]
    pub testnet: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
}

#[derive(Clap)]
struct Check {
    /// Print debug info
    #[clap(short = 'd')]
    pub debug: bool,
    /// Path to Clarinet.toml
    #[clap(long = "manifest-path")]
    pub manifest_path: Option<String>,
}

pub fn main() {
    let opts: Opts = Opts::parse();

    match opts.command {
        Command::New(project_opts) => {
            let current_path = {
                let current_dir = env::current_dir().expect("Unable to read current directory");
                current_dir.to_str().unwrap().to_owned()
            };

            let changes = generators::get_changes_for_new_project(current_path, project_opts.name);
            execute_changes(changes);
        }
        Command::Contract(subcommand) => match subcommand {
            Contract::NewContract(new_contract) => {
                let manifest_path = get_manifest_path_or_exit(new_contract.manifest_path);

                let changes = generators::get_changes_for_new_contract(
                    manifest_path,
                    new_contract.name,
                    None,
                    true,
                    vec![],
                );
                execute_changes(changes);
            }
            Contract::LinkContract(required_contract) => {
                let manifest_path = get_manifest_path_or_exit(required_contract.manifest_path);

                let change = TOMLEdition {
                    comment: format!(
                        "Adding {} as a requirement to Clarinet.toml",
                        required_contract.contract_id
                    ),
                    manifest_path,
                    contracts_to_add: HashMap::new(),
                    requirements_to_add: vec![RequirementConfig {
                        contract_id: required_contract.contract_id.clone(),
                    }],
                };
                execute_changes(vec![Changes::EditTOML(change)]);
            }
            Contract::ForkContract(fork_contract) => {
                let manifest_path = get_manifest_path_or_exit(fork_contract.manifest_path);

                println!(
                    "Resolving {} and its dependencies...",
                    fork_contract.contract_id
                );

                let settings = repl::SessionSettings::default();
                let mut session = repl::Session::new(settings);

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .max_blocking_threads(32)
                    .build()
                    .unwrap();

                let res = rt.block_on(session.resolve_link(&repl::settings::InitialLink {
                    contract_id: fork_contract.contract_id.clone(),
                    stacks_node_addr: None,
                    cache: None,
                }));
                let contracts = res.unwrap();
                let mut changes = vec![];
                for (contract_id, code, deps) in contracts.into_iter() {
                    let components: Vec<&str> = contract_id.split('.').collect();
                    let contract_name = components.last().unwrap();

                    if &contract_id == &fork_contract.contract_id {
                        let mut change_set = generators::get_changes_for_new_contract(
                            manifest_path.clone(),
                            contract_name.to_string(),
                            Some(code),
                            false,
                            vec![],
                        );
                        changes.append(&mut change_set);

                        for dep in deps.iter() {
                            let mut change_set = generators::get_changes_for_new_link(
                                manifest_path.clone(),
                                dep.clone(),
                                None,
                            );
                            changes.append(&mut change_set);
                        }
                    }
                }
                execute_changes(changes);
            }
        },
        Command::Console(cmd) => {
            let manifest_path = get_manifest_path_or_exit(cmd.manifest_path);
            let start_repl = true;
            load_session(manifest_path, start_repl, "development".into())
                .expect("Unable to start REPL");
        }
        Command::Check(cmd) => {
            let manifest_path = get_manifest_path_or_exit(cmd.manifest_path);
            let start_repl = false;
            let res = load_session(manifest_path, start_repl, "development".into());
            if let Err(e) = res {
                println!("{}", e);
                return;
            }
        }
        Command::Test(cmd) => {
            let manifest_path = get_manifest_path_or_exit(cmd.manifest_path);
            let start_repl = false;
            let res = load_session(manifest_path.clone(), start_repl, "development".into());
            if let Err(e) = res {
                println!("{}", e);
                return;
            }
            run_tests(cmd.files, cmd.coverage, cmd.watch, true, manifest_path);
        }
        Command::Run(cmd) => {
            let manifest_path = get_manifest_path_or_exit(cmd.manifest_path);
            let start_repl = false;
            let res = load_session(manifest_path.clone(), start_repl, "development".into());
            if let Err(e) = res {
                println!("{}", e);
                return;
            }
            run_tests(
                vec![cmd.script],
                false,
                false,
                cmd.allow_wallets,
                manifest_path,
            );
        }
        Command::Deploy(deploy) => {
            let manifest_path = get_manifest_path_or_exit(deploy.manifest_path);
            let start_repl = false;
            let mode = if deploy.mocknet == true {
                "mocknet"
            } else if deploy.testnet == true {
                "testnet"
            } else {
                panic!("Target deployment must be specified with --mocknet or --testnet")
            };
            let res = load_session(manifest_path, start_repl, mode.into());
            if let Err(e) = res {
                println!("{}", e);
                return;
            }
            let settings = res.unwrap();

            let mut deployers_nonces = BTreeMap::new();
            let mut deployers_lookup = BTreeMap::new();
            for account in settings.initial_accounts.iter() {
                if account.name == "deployer" {
                    deployers_lookup.insert("*", account.clone());
                }
            }

            #[derive(Deserialize, Debug)]
            struct Balance {
                balance: String,
                nonce: u64,
                balance_proof: String,
                nonce_proof: String,
            }

            let host = if mode == "mocknet" {
                "http://localhost:20443"
            } else {
                "https://stacks-node-api.testnet.stacks.co"
            };

            for initial_contract in settings.initial_contracts.iter() {
                let contract_name = initial_contract.name.clone().unwrap();

                let payload = TransactionSmartContract {
                    name: contract_name.as_str().into(),
                    code_body: StacksString::from_string(&initial_contract.code).unwrap(),
                };

                let deployer = match deployers_lookup.get(contract_name.as_str()) {
                    Some(deployer) => deployer,
                    None => deployers_lookup.get("*").unwrap(),
                };

                let bip39_seed =
                    match mnemonic::get_bip39_seed_from_mnemonic(&deployer.mnemonic, "") {
                        Ok(bip39_seed) => bip39_seed,
                        Err(_) => panic!(),
                    };
                let ext =
                    ExtendedPrivKey::derive(&bip39_seed[..], deployer.derivation.as_str()).unwrap();
                let secret_key = SecretKey::parse_slice(&ext.secret()).unwrap();
                let public_key = PublicKey::from_secret_key(&secret_key);

                let wrapped_public_key =
                    Secp256k1PublicKey::from_slice(&public_key.serialize_compressed()).unwrap();
                let wrapped_secret_key = Secp256k1PrivateKey::from_slice(&ext.secret()).unwrap();

                let anchor_mode = TransactionAnchorMode::Any;
                let tx_fee = 200 + initial_contract.code.len() as u64;

                let nonce = match deployers_nonces.get(&deployer.name) {
                    Some(nonce) => *nonce,
                    None => {
                        let request_url = format!(
                            "{host}/v2/accounts/{addr}",
                            host = host,
                            addr = deployer.address,
                        );

                        let response: Balance = reqwest::blocking::get(&request_url)
                            .expect("Unable to retrieve account")
                            .json()
                            .expect("Unable to parse contract");
                        let nonce = response.nonce;
                        deployers_nonces.insert(deployer.name.clone(), nonce);
                        nonce
                    }
                };

                let signer_addr = StacksAddress::from_public_keys(
                    0,
                    &AddressHashMode::SerializeP2PKH,
                    1,
                    &vec![wrapped_public_key],
                )
                .unwrap();

                let spending_condition =
                    TransactionSpendingCondition::Singlesig(SinglesigSpendingCondition {
                        signer: signer_addr.bytes.clone(),
                        nonce: nonce,
                        tx_fee: tx_fee,
                        hash_mode: SinglesigHashMode::P2PKH,
                        key_encoding: TransactionPublicKeyEncoding::Compressed,
                        signature: RecoverableSignature::empty(),
                    });

                let auth = TransactionAuth::Standard(spending_condition);
                let unsigned_tx = StacksTransaction {
                    version: TransactionVersion::Testnet,
                    chain_id: 0x80000000, // MAINNET=0x00000001
                    auth: auth,
                    anchor_mode: anchor_mode,
                    post_condition_mode: TransactionPostConditionMode::Deny,
                    post_conditions: vec![],
                    payload: TransactionPayload::SmartContract(payload),
                };

                let mut unsigned_tx_bytes = vec![];
                unsigned_tx
                    .consensus_serialize(&mut unsigned_tx_bytes)
                    .expect("FATAL: invalid transaction");

                let mut tx_signer = StacksTransactionSigner::new(&unsigned_tx);
                tx_signer.sign_origin(&wrapped_secret_key).unwrap();
                let signed_tx = tx_signer.get_tx().unwrap();

                let tx_bytes = signed_tx.serialize_to_vec();
                let client = reqwest::blocking::Client::new();
                let path = format!("{}/v2/transactions", host);
                let res = client
                    .post(&path)
                    .header("Content-Type", "application/octet-stream")
                    .body(tx_bytes)
                    .send()
                    .unwrap();

                if !res.status().is_success() {
                    println!("{}", res.text().unwrap());
                    panic!()
                }
                let txid: String = res.json().unwrap();

                println!(
                    "Deploying {} (txid: {}, nonce: {})",
                    contract_name, txid, nonce
                );
                deployers_nonces.insert(deployer.name.clone(), nonce + 1);
            }

            // If mocknet, we should be pulling all the links.
            // Get ordered list of contracts
            // For each contract, get the nonce of the account deploying (if unknown)
            // Create a StacksTransaction with the contract, the name.
            // Sign the transaction
            // Send the transaction
        }
    };
}

fn get_manifest_path_or_exit(path: Option<String>) -> PathBuf {
    println!("");
    if let Some(path) = path {
        let manifest_path = PathBuf::from(path);
        if !manifest_path.exists() {
            println!("Could not find Clarinet.toml");
            process::exit(1);
        }
        manifest_path
    } else {
        let mut current_dir = env::current_dir().unwrap();
        loop {
            current_dir.push("Clarinet.toml");

            if current_dir.exists() {
                break current_dir;
            }
            current_dir.pop();

            if !current_dir.pop() {
                println!("Could not find Clarinet.toml");
                process::exit(1);
            }
        }
    }
}

fn execute_changes(changes: Vec<Changes>) {
    let mut shared_config = None;
    let mut path = PathBuf::new();

    for mut change in changes.into_iter() {
        match change {
            Changes::AddFile(options) => {
                if let Ok(entry) = fs::metadata(&options.path) {
                    if entry.is_file() {
                        println!("File already exists at path {}", options.path);
                        continue;
                    }
                }
                println!("{}", options.comment);
                let mut file = File::create(options.path.clone()).expect("Unable to create file");
                file.write_all(options.content.as_bytes())
                    .expect("Unable to write file");
            }
            Changes::AddDirectory(options) => {
                println!("{}", options.comment);
                fs::create_dir_all(options.path.clone()).expect("Unable to create directory");
            }
            Changes::EditTOML(ref mut options) => {
                let mut config = match shared_config.take() {
                    Some(config) => config,
                    None => {
                        path = options.manifest_path.clone();
                        let file = File::open(path.clone()).unwrap();
                        let mut config_file_reader = BufReader::new(file);
                        let mut config_file = vec![];
                        config_file_reader.read_to_end(&mut config_file).unwrap();
                        let config_file: MainConfigFile =
                            toml::from_slice(&config_file[..]).unwrap();
                        MainConfig::from_config_file(config_file)
                    }
                };

                let mut requirements = match config.project.requirements.take() {
                    Some(requirements) => requirements,
                    None => vec![],
                };
                for requirement in options.requirements_to_add.drain(..) {
                    if !requirements.contains(&requirement) {
                        requirements.push(requirement);
                    }
                }
                config.project.requirements = Some(requirements);

                for (contract_name, contract_config) in options.contracts_to_add.drain() {
                    config.contracts.insert(contract_name, contract_config);
                }

                shared_config = Some(config);
                println!("{}", options.comment);
            }
        }
    }

    if let Some(config) = shared_config {
        let toml = toml::to_string(&config).unwrap();
        let mut file = File::create(path).unwrap();
        file.write_all(&toml.as_bytes()).unwrap();
        file.sync_all().unwrap();
    }
}
