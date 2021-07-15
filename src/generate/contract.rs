use super::changes::{Changes, FileCreation, TOMLEdition};
use crate::types::ContractConfig;
use std::{collections::HashMap, path::PathBuf};

pub struct GetChangesForNewContract {
    manifest_path: PathBuf,
    contract_name: String,
    source: Option<String>,
    changes: Vec<Changes>,
}

impl GetChangesForNewContract {
    pub fn new(manifest_path: PathBuf, contract_name: String, source: Option<String>) -> Self {
        Self {
            manifest_path,
            contract_name,
            source,
            changes: vec![],
        }
    }

    pub fn run(&mut self, include_test: bool, deps: Vec<String>) -> Vec<Changes> {
        self.create_template_contract();
        if include_test {
            self.create_template_test();
        }
        self.index_contract_in_clarinet_toml(deps);
        self.changes.clone()
    }

    fn create_template_contract(&mut self) {
        let content = if let Some(ref source) = self.source {
            source.to_string()
        } else {
            format!(
                r#"
;; {}
;; <add a description here>

;; constants
;;

;; data maps and vars
;;

;; private functions
;;

;; public functions
;;
"#,
                self.contract_name
            )
        };

        let name = format!("{}.clar", self.contract_name);
        let project_path = {
            let mut p = self.manifest_path.clone();
            p.pop();
            p
        };
        let path = format!("{}/contracts/{}", project_path.to_string_lossy(), name);
        let change = FileCreation {
            comment: format!("Creating file contracts/{}", name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_template_test(&mut self) {
        let content = format!(
            r#"
import {{ Clarinet, Tx, Chain, Account, types }} from 'https://deno.land/x/clarinet@v0.14.0/index.ts';
import {{ assertEquals }} from 'https://deno.land/std@0.90.0/testing/asserts.ts';

Clarinet.test({{
    name: "Ensure that <...>",
    async fn(chain: Chain, accounts: Map<string, Account>) {{
        let block = chain.mineBlock([
            /* 
             * Add transactions with: 
             * Tx.contractCall(...)
            */
        ]);
        assertEquals(block.receipts.length, 0);
        assertEquals(block.height, 2);

        block = chain.mineBlock([
            /* 
             * Add transactions with: 
             * Tx.contractCall(...)
            */
        ]);
        assertEquals(block.receipts.length, 0);
        assertEquals(block.height, 3);
    }},
}});
"#
        );
        let project_path = {
            let mut p = self.manifest_path.clone();
            p.pop();
            p
        };
        let name = format!("{}_test.ts", self.contract_name);
        let path = format!("{}/tests/{}", project_path.to_string_lossy(), name);
        let change = FileCreation {
            comment: format!("Creating file tests/{}", name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn index_contract_in_clarinet_toml(&mut self, deps: Vec<String>) {
        let contract_file_name = format!("{}.clar", self.contract_name);
        let manifest_path = self.manifest_path.clone();

        let contract_config = ContractConfig {
            depends_on: deps,
            path: format!("contracts/{}", contract_file_name),
        };
        let mut contracts_to_add = HashMap::new();
        contracts_to_add.insert(self.contract_name.clone(), contract_config);

        let change = TOMLEdition {
            comment: format!("Adding contract {} to Clarinet.toml", self.contract_name),
            manifest_path,
            contracts_to_add,
            requirements_to_add: vec![],
        };
        self.changes.push(Changes::EditTOML(change));
    }
}
