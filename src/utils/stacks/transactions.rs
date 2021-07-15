use std::convert::TryInto;

use clarity_repl::clarity::codec::transaction::*;
use clarity_repl::clarity::codec::StacksMessageCodec;
use clarity_repl::clarity::types::{QualifiedContractIdentifier, Value};
use clarity_repl::clarity::util::{
    address::AddressHashMode,
    secp256k1::{Secp256k1PrivateKey, Secp256k1PublicKey},
    StacksAddress,
};

pub fn build_contrat_call_transaction(
    contract_id: String,
    function_name: String,
    args: Vec<Value>,
    nonce: u64,
    fee: u64,
    sender_secret_key: &[u8],
) -> StacksTransaction {
    let contract_id =
        QualifiedContractIdentifier::parse(&contract_id).expect("Contract identifier invalid");

    let payload = TransactionContractCall {
        address: contract_id.issuer.into(),
        contract_name: contract_id.name.into(),
        function_name: function_name.try_into().unwrap(),
        function_args: args,
    };

    let secret_key = Secp256k1PrivateKey::from_slice(sender_secret_key).unwrap();
    let mut public_key = Secp256k1PublicKey::from_private(&secret_key);
    public_key.set_compressed(true);

    let anchor_mode = TransactionAnchorMode::Any;
    let signer_addr =
        StacksAddress::from_public_keys(0, &AddressHashMode::SerializeP2PKH, 1, &vec![public_key])
            .unwrap();

    let spending_condition = TransactionSpendingCondition::Singlesig(SinglesigSpendingCondition {
        signer: signer_addr.bytes.clone(),
        nonce: nonce,
        tx_fee: fee,
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
        post_condition_mode: TransactionPostConditionMode::Allow,
        post_conditions: vec![],
        payload: TransactionPayload::ContractCall(payload),
    };

    let mut unsigned_tx_bytes = vec![];
    unsigned_tx
        .consensus_serialize(&mut unsigned_tx_bytes)
        .expect("FATAL: invalid transaction");

    let mut tx_signer = StacksTransactionSigner::new(&unsigned_tx);
    tx_signer.sign_origin(&secret_key).unwrap();
    let signed_tx = tx_signer.get_tx().unwrap();

    signed_tx
}
