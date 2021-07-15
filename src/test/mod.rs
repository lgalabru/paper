#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(unused_must_use)]

use crate::utils;
use std::path::PathBuf;

mod deno;

pub fn run_tests(
    files: Vec<String>,
    include_coverage: bool,
    watch: bool,
    allow_wallets: bool,
    manifest_path: PathBuf,
) {
    match block_on(deno::do_run_tests(
        files,
        include_coverage,
        watch,
        allow_wallets,
        manifest_path,
    )) {
        Err(e) => std::process::exit(1),
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

// struct ClaritestTransaction {
// }

// struct ClaritestBlock {
// }

// struct ClaritestChain {
// }

// struct ClaritestAccount {
// }

// impl ClaritestChain {

//     fn new() -> ClaritestChain {
//         ClaritestChain {
//         }
//     }

//     fn test() {
//         let config = ClarinetConfig::new();
//         let chain = ClaritestChain::new(config);
//         chain.start();

//     }
// }

// #[claritest()]
// fn test_box_btc(chain: ClaritestChain, accounts: Hashmap<String, ClaritestAccount>) {
//     let block = chain.mine_block(vec![
//         tx!("(contract-call? 'ST000000000000000000002AMW42H.bbtc create-box size fee)"),
//     ]);

//     let res = chain.read("(contract-call? 'ST000000000000000000002AMW42H.bbtc create-box size fee)");
// }
