use crate::publish::Network;
use crate::types::{ChainConfig, MainConfig};
use clarity_repl::{repl, Terminal};
use std::fs;
use std::path::PathBuf;

pub fn load_session(
    manifest_path: PathBuf,
    start_repl: bool,
    env: Network,
) -> Result<repl::SessionSettings, String> {
    let mut settings = repl::SessionSettings::default();

    let mut project_path = manifest_path.clone();
    project_path.pop();

    let mut chain_config_path = project_path.clone();
    // chain_config_path.pop();
    chain_config_path.push("settings");

    chain_config_path.push(match env {
        Network::Devnet => "Devnet.toml",
        Network::Testnet => "Testnet.toml",
        Network::Mainnet => "Mainnet.toml",
    });

    let mut project_config = MainConfig::from_path(&manifest_path);
    let chain_config = ChainConfig::from_path(&chain_config_path);

    let mut deployer_address = None;
    let mut initial_deployer = None;

    settings.node = chain_config
        .network
        .node_rpc_address
        .clone()
        .take()
        .unwrap_or(match env {
            Network::Devnet => "http://127.0.0.1:20443".into(),
            Network::Testnet => "https://stacks-node-api.testnet.stacks.co".into(),
            Network::Mainnet => "https://stacks-node-api.mainnet.stacks.co".into(),
        });

    for (name, account) in chain_config.accounts.iter() {
        let account = repl::settings::Account {
            name: name.clone(),
            balance: account.balance,
            address: account.address.clone(),
            mnemonic: account.mnemonic.clone(),
            derivation: account.derivation.clone(),
        };
        if name == "deployer" {
            initial_deployer = Some(account.clone());
            deployer_address = Some(account.address.clone());
        }
        settings.initial_accounts.push(account);
    }

    for (name, config) in project_config.ordered_contracts().iter() {
        let mut contract_path = project_path.clone();
        contract_path.push(&config.path);

        let code = match fs::read_to_string(&contract_path) {
            Ok(code) => code,
            Err(err) => {
                return Err(format!(
                    "Error: unable to read {:?}: {}",
                    contract_path, err
                ))
            }
        };

        settings
            .initial_contracts
            .push(repl::settings::InitialContract {
                code: code,
                path: contract_path.to_str().unwrap().into(),
                name: Some(name.clone()),
                deployer: deployer_address.clone(),
            });
    }

    let links = match project_config.project.requirements.take() {
        Some(links) => links,
        None => vec![],
    };

    for link_config in links.iter() {
        settings.initial_links.push(repl::settings::InitialLink {
            contract_id: link_config.contract_id.clone(),
            stacks_node_addr: None,
            cache: None,
        });
    }

    settings.include_boot_contracts =
        vec!["pox".to_string(), "costs".to_string(), "bns".to_string()];
    settings.initial_deployer = initial_deployer;

    if start_repl {
        let mut terminal = Terminal::new(settings.clone());
        terminal.start();
    } else {
        let mut session = repl::Session::new(settings.clone());
        match session.check() {
            Err(message) => {
                println!("Error: {}", message);
                std::process::exit(1);
            }
            _ => {}
        };
    }
    Ok(settings)
}
