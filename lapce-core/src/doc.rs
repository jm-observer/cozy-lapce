use std::path::PathBuf;

use lapce_rpc::buffer::BufferId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub enum DocContent {
    /// A file at some location. This can be a remote path.
    File { path: PathBuf, read_only: bool },
    /// A local document, which doens't need to be sync to the disk.
    Local,
    /// A document of an old version in the source control
    History(DocHistory),
    /// A new file which doesn't exist in the file system
    Scratch { id: BufferId, name: String }
}

impl DocContent {
    pub fn is_local(&self) -> bool {
        matches!(self, DocContent::Local)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, DocContent::File { .. })
    }

    pub fn read_only(&self) -> bool {
        match self {
            DocContent::File { read_only, .. } => *read_only,
            DocContent::Local => false,
            DocContent::History(_) => true,
            DocContent::Scratch { .. } => false
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            DocContent::File { path, .. } => Some(path),
            DocContent::Local => None,
            DocContent::History(_) => None,
            DocContent::Scratch { .. } => None
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct DocHistory {
    pub path:    PathBuf,
    pub version: String
}
