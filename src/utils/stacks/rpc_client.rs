use clarity_repl::clarity::codec::{StacksMessageCodec, StacksTransaction};
use clarity_repl::clarity::types::Value;
use clarity_repl::clarity::util::hash::{bytes_to_hex, hex_bytes};
use std::io::Cursor;

#[derive(Debug)]
pub enum RpcError {
    Generic,
}

pub struct StacksRpc {
    pub url: String,
}

pub struct PostTransactionResult {
    pub txid: String,
}

pub struct CallReadOnlyFnResult {
    pub result: Value,
}

#[derive(Deserialize, Debug)]
struct Balance {
    balance: String,
    nonce: u64,
    balance_proof: String,
    nonce_proof: String,
}

impl StacksRpc {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub fn post_transaction(
        &self,
        transaction: StacksTransaction,
    ) -> Result<PostTransactionResult, RpcError> {
        let tx = transaction.serialize_to_vec();
        let client = reqwest::blocking::Client::new();
        let path = format!("{}/v2/transactions", self.url);
        let res = client
            .post(&path)
            .header("Content-Type", "application/octet-stream")
            .body(tx)
            .send()
            .unwrap();

        if !res.status().is_success() {
            println!("{}", res.text().unwrap());
            return Err(RpcError::Generic);
        }

        let txid: String = res.json().unwrap();
        let res = PostTransactionResult { txid };
        Ok(res)
    }

    pub fn get_nonce(&self, address: String) -> Result<u64, RpcError> {
        let request_url = format!("{}/v2/accounts/{addr}", self.url, addr = address,);

        let res: Balance = reqwest::blocking::get(&request_url)
            .expect("Unable to retrieve account")
            .json()
            .expect("Unable to parse contract");
        let nonce = res.nonce;
        Ok(nonce)
    }

    pub fn call_read_only_fn(
        &self,
        contract_addr: String,
        contract_name: String,
        method: String,
        args: Vec<Value>,
        sender: String,
    ) -> Result<Value, RpcError> {
        let client = reqwest::blocking::Client::new();
        let path = format!(
            "{}/v2/contracts/call-read/{}/{}/{}",
            self.url, contract_addr, contract_name, method
        );

        let arguments = args
            .iter()
            .map(|a| bytes_to_hex(&a.serialize_to_vec()))
            .collect::<Vec<_>>();
        let res = client
            .post(&path)
            .json(&json!({
                "sender": sender,
                "arguments": arguments,
            }))
            .send()
            .unwrap();

        if !res.status().is_success() {
            println!("{}", res.text().unwrap());
            return Err(RpcError::Generic);
        }

        #[derive(Deserialize, Debug)]
        struct ReadOnlyCallResult {
            okay: bool,
            result: String,
        }

        let mut response: ReadOnlyCallResult = res.json().unwrap();
        if response.okay {
            // Removing the 0x prefix
            let clar_val = response.result.split_off(2);
            let bytes = hex_bytes(&clar_val).unwrap();
            let mut cursor = Cursor::new(&bytes);
            let value = Value::consensus_deserialize(&mut cursor).unwrap();
            Ok(value)
        } else {
            Err(RpcError::Generic)
        }
    }
}
