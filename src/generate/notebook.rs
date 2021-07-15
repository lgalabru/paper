use std::path::PathBuf;

use super::changes::Changes;

#[allow(dead_code)]
pub struct GetChangesForNewNotebook {
    pub manifest_path: PathBuf,
    pub notebook_name: String,
    changes: Vec<Changes>,
}

impl GetChangesForNewNotebook {
    #[allow(dead_code)]
    pub fn new(manifest_path: PathBuf, notebook_name: String) -> Self {
        Self {
            manifest_path,
            notebook_name,
            changes: vec![],
        }
    }

    #[allow(dead_code)]
    pub fn run(&self) -> Vec<Changes> {
        self.changes.clone()
    }
}
