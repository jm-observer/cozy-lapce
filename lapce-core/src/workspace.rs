use std::{collections::HashMap, fmt::Display, path::PathBuf};

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::{debug::LapceBreakpoint, main_split::SplitInfo, panel::PanelInfo};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct SshHost {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<usize>,
}

impl SshHost {
    pub fn from_string(s: &str) -> Self {
        let mut whole_splits = s.split(':');
        let splits = whole_splits
            .next()
            .unwrap()
            .split('@')
            .collect::<Vec<&str>>();
        let mut splits = splits.iter().rev();
        let host = splits.next().unwrap().to_string();
        let user = splits.next().map(|s| s.to_string());
        let port = whole_splits.next().and_then(|s| s.parse::<usize>().ok());
        Self { user, host, port }
    }

    pub fn user_host(&self) -> String {
        if let Some(user) = self.user.as_ref() {
            format!("{user}@{}", self.host)
        } else {
            self.host.clone()
        }
    }
}

impl Display for SshHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(user) = self.user.as_ref() {
            write!(f, "{user}@")?;
        }
        write!(f, "{}", self.host)?;
        if let Some(port) = self.port {
            write!(f, ":{port}")?;
        }
        Ok(())
    }
}

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct WslHost {
    pub host: String,
}

#[cfg(windows)]
impl Display for WslHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.host)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LapceWorkspaceType {
    Local,
    RemoteSSH(SshHost),
    #[cfg(windows)]
    RemoteWSL(WslHost),
}

impl LapceWorkspaceType {
    pub fn is_local(&self) -> bool {
        matches!(self, LapceWorkspaceType::Local)
    }

    pub fn is_remote(&self) -> bool {
        use LapceWorkspaceType::*;

        #[cfg(not(windows))]
        return matches!(self, RemoteSSH(_));

        #[cfg(windows)]
        return matches!(self, RemoteSSH(_) | RemoteWSL(_));
    }
}

impl std::fmt::Display for LapceWorkspaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LapceWorkspaceType::Local => f.write_str("Local"),
            LapceWorkspaceType::RemoteSSH(remote) => {
                write!(f, "ssh://{remote}")
            },
            #[cfg(windows)]
            LapceWorkspaceType::RemoteWSL(remote) => {
                write!(f, "{remote} (WSL)")
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LapceWorkspace {
    kind:      LapceWorkspaceType,
    path:      Option<PathBuf>,
    last_open: u64,
}

impl LapceWorkspace {
    pub fn new(
        kind: LapceWorkspaceType,
        path: Option<PathBuf>,
        last_open: u64,
    ) -> Self {
        Self {
            kind,
            path,
            last_open,
        }
    }

    #[cfg(windows)]
    pub fn new_remote_wsl(wsl: WslHost) -> Self {
        Self::new(LapceWorkspaceType::RemoteWSL(wsl), None, 0)
    }

    pub fn new_ssh(ssh: SshHost) -> Self {
        Self::new(LapceWorkspaceType::RemoteSSH(ssh), None, 0)
    }

    pub fn new_with_path(path: Option<PathBuf>) -> Self {
        Self {
            path,
            ..Default::default()
        }
    }

    pub fn kind(&self) -> &LapceWorkspaceType {
        &self.kind
    }

    pub fn last_open(&self) -> u64 {
        self.last_open
    }

    pub fn update_last_open(&mut self, last_open: u64) {
        self.last_open = last_open;
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn display(&self) -> Option<String> {
        let path = self.path.as_ref()?;
        let path = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();
        let remote = match &self.kind {
            LapceWorkspaceType::Local => String::new(),
            LapceWorkspaceType::RemoteSSH(remote) => {
                format!(" [SSH: {}]", remote.host)
            },
            #[cfg(windows)]
            LapceWorkspaceType::RemoteWSL(remote) => {
                format!(" [WSL: {}]", remote.host)
            },
        };
        Some(format!("{path}{remote}"))
    }

    pub fn watch_project_setting(
        &self,
        watcher: &std::sync::Arc<RwLock<notify::RecommendedWatcher>>,
    ) {
        if let Some(path) = self.project_setting() {
            if let Err(e) = watcher
                .write_arc()
                .watch(&path, RecursiveMode::NonRecursive)
            {
                log::error!("{:?}", e);
            }
        }
    }

    pub fn unwatch_project_setting(
        &self,
        watcher: &std::sync::Arc<RwLock<notify::RecommendedWatcher>>,
    ) {
        if let Some(path) = self.project_setting() {
            if let Err(e) = watcher.write_arc().unwatch(&path) {
                log::error!("{:?}", e);
            }
        }
    }

    pub fn project_setting(&self) -> Option<PathBuf> {
        if let Some(path) = &self.path {
            if path.exists() {
                return Some(path.join("").join(""));
            }
        }
        None
    }

    pub fn run_and_debug_path(&self) -> Result<Option<PathBuf>> {
        Ok(if let Some(path) = self.path.as_ref() {
            let path = path.join(".lapce").join("run.toml");
            Some(path)
        } else {
            None
        })
    }
}

impl Default for LapceWorkspace {
    fn default() -> Self {
        Self {
            kind:      LapceWorkspaceType::Local,
            path:      None,
            last_open: 0,
        }
    }
}

impl std::fmt::Display for LapceWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}",
            self.kind,
            self.path.as_ref().and_then(|p| p.to_str()).unwrap_or("")
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub split:       SplitInfo,
    pub panel:       PanelInfo,
    pub breakpoints: HashMap<PathBuf, Vec<LapceBreakpoint>>,
}
