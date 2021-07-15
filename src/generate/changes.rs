use crate::types::{ContractConfig, RequirementConfig};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Debug)]
pub struct FileCreation {
    pub comment: String,
    pub name: String,
    pub content: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct DirectoryCreation {
    pub comment: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct TOMLEdition {
    pub comment: String,
    pub manifest_path: PathBuf,
    pub contracts_to_add: HashMap<String, ContractConfig>,
    pub requirements_to_add: Vec<RequirementConfig>,
}

#[derive(Clone, Debug)]
pub enum Changes {
    AddFile(FileCreation),
    AddDirectory(DirectoryCreation),
    EditTOML(TOMLEdition),
}
