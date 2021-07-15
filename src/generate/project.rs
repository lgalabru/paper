use super::changes::{Changes, DirectoryCreation, FileCreation};

pub struct GetChangesForNewProject {
    project_path: String,
    project_name: String,
    changes: Vec<Changes>,
}

impl GetChangesForNewProject {
    pub fn new(project_path: String, project_name: String) -> Self {
        Self {
            project_path,
            project_name,
            changes: vec![],
        }
    }

    pub fn run(&mut self) -> Vec<Changes> {
        self.create_root_directory();
        self.create_contracts_directory();
        self.create_settings_directory();
        self.create_tests_directory();
        self.create_clarinet_toml();
        self.create_environment_mainnet_toml();
        self.create_environment_testnet_toml();
        self.create_environment_devnet_toml();
        self.create_vscode_directory();
        self.create_gitignore();
        self.changes.clone()
    }

    fn create_root_directory(&mut self) {
        let dir = format!("{}/{}", self.project_path, self.project_name);
        let change = DirectoryCreation {
            comment: format!("Creating directory {}", self.project_name),
            name: self.project_name.clone(),
            path: dir,
        };
        self.changes.push(Changes::AddDirectory(change));
    }

    #[allow(dead_code)]
    fn create_clients_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("clients")));
    }

    fn create_contracts_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("contracts")));
    }

    #[allow(dead_code)]
    fn create_notebooks_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("notebooks")));
    }

    #[allow(dead_code)]
    fn create_scripts_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("scripts")));
    }

    fn create_settings_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("settings")));
    }

    fn create_tests_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!("tests")));
    }

    fn create_vscode_directory(&mut self) {
        self.changes
            .push(self.get_changes_for_new_root_dir(format!(".vscode")));
        let content = format!(
            r#"
{{
    "deno.enable": true,
}}
"#
        );
        let name = format!("settings.json");
        let path = format!(
            "{}/{}/.vscode/{}",
            self.project_path, self.project_name, name
        );
        let change = FileCreation {
            comment: format!("Creating file {}/.vscode/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_gitignore(&mut self) {
        let content = format!(
            r#"
settings/Mainnet.toml
settings/Testnet.toml
history.txt
"#,
        );
        let name = format!(".gitignore");
        let path = format!("{}/{}/{}", self.project_path, self.project_name, name);
        let change = FileCreation {
            comment: format!("Creating file {}/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_clarinet_toml(&mut self) {
        let content = format!(
            r#"
[project]
name = "{}"

[contracts]

[notebooks]
"#,
            self.project_name
        );
        let name = format!("Clarinet.toml");
        let path = format!("{}/{}/{}", self.project_path, self.project_name, name);
        let change = FileCreation {
            comment: format!("Creating file {}/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_environment_testnet_toml(&mut self) {
        let content = format!(
            r#"[network]
name = "testnet"
node_rpc_address = "https://stacks-node-api.testnet.stacks.co"

[accounts.deployer]
mnemonic = "<YOUR PRIVATE TESTNET MNEMONIC HERE>"
"#
        );
        let name = format!("Testnet.toml");
        let path = format!(
            "{}/{}/settings/{}",
            self.project_path, self.project_name, name
        );
        let change = FileCreation {
            comment: format!("Creating file {}/settings/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_environment_mainnet_toml(&mut self) {
        let content = format!(
            r#"[network]
name = "mainnet"
node_rpc_address = "https://stacks-node-api.mainnet.stacks.co"

[accounts.deployer]
mnemonic = "<YOUR PRIVATE MAINNET MNEMONIC HERE>"
"#
        );
        let name = format!("Mainnet.toml");
        let path = format!(
            "{}/{}/settings/{}",
            self.project_path, self.project_name, name
        );
        let change = FileCreation {
            comment: format!("Creating file {}/settings/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn create_environment_devnet_toml(&mut self) {
        let content = format!(
            r#"[network]
name = "devnet"

[accounts.deployer]
mnemonic = "twice kind fence tip hidden tilt action fragile skin nothing glory cousin green tomorrow spring wrist shed math olympic multiply hip blue scout claw"
balance = 100_000_000_000_000
# secret_key: 753b7cc01a1a2e86221266a154af739463fce51219d97e4f856cd7200c3bd2a601
# stx_address: ST1PQHQKV0RJXZFY1DGX8MNSNYVE3VGZJSRTPGZGM
# btc_address: mqVnk6NPRdhntvfm4hh9vvjiRkFDUuSYsH

[accounts.wallet_1]
mnemonic = "sell invite acquire kitten bamboo drastic jelly vivid peace spawn twice guilt pave pen trash pretty park cube fragile unaware remain midnight betray rebuild"
balance = 100_000_000_000_000
# secret_key: 7287ba251d44a4d3fd9276c88ce34c5c52a038955511cccaf77e61068649c17801
# stx_address: ST1SJ3DTE5DN7X54YDH5D64R3BCB6A2AG2ZQ8YPD5
# btc_address: mr1iPkD9N3RJZZxXRk7xF9d36gffa6exNC

[accounts.wallet_2]
mnemonic = "hold excess usual excess ring elephant install account glad dry fragile donkey gaze humble truck breeze nation gasp vacuum limb head keep delay hospital"
balance = 100_000_000_000_000
# secret_key: 530d9f61984c888536871c6573073bdfc0058896dc1adfe9a6a10dfacadc209101
# stx_address: ST2CY5V39NHDPWSXMW9QDT3HC3GD6Q6XX4CFRK9AG
# btc_address: muYdXKmX9bByAueDe6KFfHd5Ff1gdN9ErG

[accounts.wallet_3]
mnemonic = "cycle puppy glare enroll cost improve round trend wrist mushroom scorpion tower claim oppose clever elephant dinosaur eight problem before frozen dune wagon high"
balance = 100_000_000_000_000
# secret_key: d655b2523bcd65e34889725c73064feb17ceb796831c0e111ba1a552b0f31b3901
# stx_address: ST2JHG361ZXG51QTKY2NQCVBPPRRE2KZB1HR05NNC
# btc_address: mvZtbibDAAA3WLpY7zXXFqRa3T4XSknBX7

[accounts.wallet_4]
mnemonic = "board list obtain sugar hour worth raven scout denial thunder horse logic fury scorpion fold genuine phrase wealth news aim below celery when cabin"
balance = 100_000_000_000_000
# secret_key: f9d7206a47f14d2870c163ebab4bf3e70d18f5d14ce1031f3902fbbc894fe4c701
# stx_address: ST2NEB84ASENDXKYGJPQW86YXQCEFEX2ZQPG87ND
# btc_address: mg1C76bNTutiCDV3t9nWhZs3Dc8LzUufj8

[accounts.wallet_5]
mnemonic = "hurry aunt blame peanut heavy update captain human rice crime juice adult scale device promote vast project quiz unit note reform update climb purchase"
balance = 100_000_000_000_000
# secret_key: 3eccc5dac8056590432db6a35d52b9896876a3d5cbdea53b72400bc9c2099fe801
# stx_address: ST2REHHS5J3CERCRBEPMGH7921Q6PYKAADT7JP2VB
# btc_address: mweN5WVqadScHdA81aATSdcVr4B6dNokqx

[accounts.wallet_6]
mnemonic = "area desk dutch sign gold cricket dawn toward giggle vibrant indoor bench warfare wagon number tiny universe sand talk dilemma pottery bone trap buddy"
balance = 100_000_000_000_000
# secret_key: 7036b29cb5e235e5fd9b09ae3e8eec4404e44906814d5d01cbca968a60ed4bfb01
# stx_address: ST3AM1A56AK2C1XAFJ4115ZSV26EB49BVQ10MGCS0
# btc_address: mzxXgV6e4BZSsz8zVHm3TmqbECt7mbuErt

[accounts.wallet_7]
mnemonic = "prevent gallery kind limb income control noise together echo rival record wedding sense uncover school version force bleak nuclear include danger skirt enact arrow"
balance = 100_000_000_000_000
# secret_key: b463f0df6c05d2f156393eee73f8016c5372caa0e9e29a901bb7171d90dc4f1401
# stx_address: ST3PF13W7Z0RRM42A8VZRVFQ75SV1K26RXEP8YGKJ
# btc_address: n37mwmru2oaVosgfuvzBwgV2ysCQRrLko7

[accounts.wallet_8]
mnemonic = "female adjust gallery certain visit token during great side clown fitness like hurt clip knife warm bench start reunion globe detail dream depend fortune"
balance = 100_000_000_000_000
# secret_key: 6a1a754ba863d7bab14adbbc3f8ebb090af9e871ace621d3e5ab634e1422885e01
# stx_address: ST3NBRSFKX28FQ2ZJ1MAKX58HKHSDGNV5N7R21XCP
# btc_address: n2v875jbJ4RjBnTjgbfikDfnwsDV5iUByw

[accounts.wallet_9]
mnemonic = "shadow private easily thought say logic fault paddle word top book during ignore notable orange flight clock image wealth health outside kitten belt reform"
balance = 100_000_000_000_000
# secret_key: de433bdfa14ec43aa1098d5be594c8ffb20a31485ff9de2923b2689471c401b801
# stx_address: STNHKEPYEPJ8ET55ZZ0M5A34J0R3N5FM2CMMMAZ6
# btc_address: mjSrB3wS4xab3kYqFktwBzfTdPg367ZJ2d
"#
        );
        let name = format!("Devnet.toml");
        let path = format!(
            "{}/{}/settings/{}",
            self.project_path, self.project_name, name
        );
        let change = FileCreation {
            comment: format!("Creating file {}/settings/{}", self.project_name, name),
            name,
            content,
            path,
        };
        self.changes.push(Changes::AddFile(change));
    }

    fn get_changes_for_new_root_dir(&self, name: String) -> Changes {
        let dir = format!("{}/{}", self.project_name, name);
        let change = DirectoryCreation {
            comment: format!("Creating directory {}/{}", self.project_name, name),
            name,
            path: dir,
        };
        Changes::AddDirectory(change)
    }
}
