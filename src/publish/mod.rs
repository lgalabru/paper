use crate::poke::load_session;
use crate::utils::mnemonic;
use clarity_repl::clarity::codec::transaction::{
    StacksTransaction, StacksTransactionSigner, TransactionAnchorMode, TransactionAuth,
    TransactionPayload, TransactionPostConditionMode, TransactionPublicKeyEncoding,
    TransactionSmartContract, TransactionSpendingCondition,
};
use clarity_repl::clarity::codec::StacksMessageCodec;
use clarity_repl::clarity::{
    codec::{
        transaction::{
            RecoverableSignature, SinglesigHashMode, SinglesigSpendingCondition, TransactionVersion,
        },
        StacksString,
    },
    util::{
        address::AddressHashMode,
        secp256k1::{Secp256k1PrivateKey, Secp256k1PublicKey},
        StacksAddress,
    },
};
use secp256k1::{PublicKey, SecretKey};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tiny_hderive::bip32::ExtendedPrivKey;

#[derive(Deserialize, Debug)]
struct Balance {
    balance: String,
    nonce: u64,
    balance_proof: String,
    nonce_proof: String,
}

#[allow(dead_code)]
pub enum Network {
    Devnet,
    Testnet,
    Mainnet,
}

pub fn publish_contracts(manifest_path: PathBuf, network: Network) -> Result<Vec<String>, String> {
    let start_repl = false;
    let settings = load_session(manifest_path, start_repl, network)?;
    let mut results = vec![];
    let mut deployers_nonces = BTreeMap::new();
    let mut deployers_lookup = BTreeMap::new();
    for account in settings.initial_accounts.iter() {
        if account.name == "deployer" {
            deployers_lookup.insert("*", account.clone());
        }
    }

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

        let bip39_seed = match mnemonic::get_bip39_seed_from_mnemonic(&deployer.mnemonic, "") {
            Ok(bip39_seed) => bip39_seed,
            Err(_) => panic!(),
        };
        let ext = ExtendedPrivKey::derive(&bip39_seed[..], deployer.derivation.as_str()).unwrap();
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
                    host = settings.node,
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
            chain_id: 0x80000000, // MAINNET=0x00000001 TODO(ludo): mainnet handling
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
        let path = format!("{}/v2/transactions", settings.node);
        let res = client
            .post(&path)
            .header("Content-Type", "application/octet-stream")
            .body(tx_bytes)
            .send()
            .unwrap();

        if !res.status().is_success() {
            return Err(format!("{}", res.text().unwrap()));
        }
        let txid: String = res.json().unwrap();

        results.push(format!(
            "Contract {} broadcasted in mempool (txid: {}, nonce: {})",
            contract_name, txid, nonce
        ));
        deployers_nonces.insert(deployer.name.clone(), nonce + 1);
    }
    // If devnet, we should be pulling all the links.
    // Get ordered list of contracts
    // For each contract, get the nonce of the account deploying (if unknown)
    // Create a StacksTransaction with the contract, the name.
    // Sign the transaction
    // Send the transaction
    Ok(results)
}
