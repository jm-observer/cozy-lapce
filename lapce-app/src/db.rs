use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use floem::{peniko::kurbo::Vec2, reactive::SignalGet};
use lapce_core::{
    panel::{PanelKind, PanelOrder},
    workspace::{LapceWorkspace, WorkspaceInfo},
};
use lapce_rpc::plugin::VoltID;
use sha2::{Digest, Sha256};

use crate::{
    app::{AppData, AppInfo},
    doc::DocInfo,
    local_task::{LocalNotification, LocalTaskRequester},
    window::{WindowData, WindowInfo},
    window_workspace::WindowWorkspaceData,
};

const APP: &str = "app";
const WINDOW: &str = "window";
const WORKSPACE_INFO: &str = "workspace_info";
const WORKSPACE_FILES: &str = "workspace_files";
const PANEL_ORDERS: &str = "panel_orders";
const DISABLED_VOLTS: &str = "disabled_volts";
const RECENT_WORKSPACES: &str = "recent_workspaces";

pub enum SaveEvent {
    App(AppInfo),
    Workspace(Arc<LapceWorkspace>, WorkspaceInfo),
    RecentWorkspace(Arc<LapceWorkspace>),
    Doc(DocInfo),
    DisabledVolts(Vec<VoltID>),
    WorkspaceDisabledVolts(Arc<LapceWorkspace>, Vec<VoltID>),
    PanelOrder(PanelOrder),
}

#[derive(Clone)]
pub struct LapceDb {
    folder:           PathBuf,
    workspace_folder: PathBuf, // save_tx:          Sender<SaveEvent>,
}

impl LapceDb {
    pub fn new(config_directory: &Path) -> Result<Self> {
        let folder = config_directory.join("db");
        let workspace_folder = folder.join("workspaces");
        if let Err(err) = std::fs::create_dir_all(&workspace_folder) {
            log::error!("{:?}", err);
        }

        let db = Self {
            workspace_folder,
            folder,
        };
        // let local_db = db.clone();
        // std::thread::Builder::new()
        //     .name("SaveEventHandler".to_owned())
        //     .spawn(move || -> Result<()> {
        //         loop {
        //             let event = save_rx.recv()?;

        //     })
        //     .unwrap();
        Ok(db)
    }

    pub fn get_disabled_volts(&self) -> Result<Vec<VoltID>> {
        let volts = std::fs::read_to_string(self.folder.join(DISABLED_VOLTS))?;
        let volts: Vec<VoltID> = serde_json::from_str(&volts)?;
        Ok(volts)
    }

    pub fn save_disabled_volts(
        &self,
        volts: Vec<VoltID>,
        requester: &LocalTaskRequester,
    ) {
        requester.notification(LocalNotification::DbSaveEvent(
            SaveEvent::DisabledVolts(volts),
        ));
    }

    pub fn save_workspace_disabled_volts(
        &self,
        workspace: Arc<LapceWorkspace>,
        volts: Vec<VoltID>,
        requester: &LocalTaskRequester,
    ) {
        requester.notification(LocalNotification::DbSaveEvent(
            SaveEvent::WorkspaceDisabledVolts(workspace, volts),
        ));
    }

    pub fn insert_disabled_volts(&self, volts: Vec<VoltID>) -> Result<()> {
        let volts = serde_json::to_string_pretty(&volts)?;
        std::fs::write(self.folder.join(DISABLED_VOLTS), volts)?;
        Ok(())
    }

    pub fn insert_workspace_disabled_volts(
        &self,
        workspace: Arc<LapceWorkspace>,
        volts: Vec<VoltID>,
    ) -> Result<()> {
        let folder = self
            .workspace_folder
            .join(workspace_folder_name(&workspace));
        if let Err(err) = std::fs::create_dir_all(&folder) {
            log::error!("{:?}", err);
        }

        let volts = serde_json::to_string_pretty(&volts)?;
        std::fs::write(folder.join(DISABLED_VOLTS), volts)?;
        Ok(())
    }

    pub fn get_workspace_disabled_volts(
        &self,
        workspace: &LapceWorkspace,
    ) -> Result<Vec<VoltID>> {
        let folder = self.workspace_folder.join(workspace_folder_name(workspace));
        let volts = std::fs::read_to_string(folder.join(DISABLED_VOLTS))?;
        let volts: Vec<VoltID> = serde_json::from_str(&volts)?;
        Ok(volts)
    }

    pub fn recent_workspaces(&self) -> Result<Vec<LapceWorkspace>> {
        let workspaces =
            std::fs::read_to_string(self.folder.join(RECENT_WORKSPACES))?;
        let workspaces: Vec<LapceWorkspace> = serde_json::from_str(&workspaces)?;
        Ok(workspaces)
    }

    pub fn update_recent_workspace(
        &self,
        workspace: Arc<LapceWorkspace>,
        requester: &LocalTaskRequester,
    ) {
        if workspace.path().is_none() {
            return;
        }
        requester.notification(LocalNotification::DbSaveEvent(
            SaveEvent::RecentWorkspace(workspace),
        ));
    }

    pub fn insert_recent_workspace(
        &self,
        workspace: Arc<LapceWorkspace>,
    ) -> Result<()> {
        let mut workspaces = self.recent_workspaces().unwrap_or_default();

        let mut exits = false;
        for w in workspaces.iter_mut() {
            if w.path() == workspace.path() && w.kind() == workspace.kind() {
                w.update_last_open(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                );
                exits = true;
                break;
            }
        }
        if !exits {
            let mut workspace = workspace.as_ref().clone();
            workspace.update_last_open(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
            workspaces.push(workspace);
        }
        workspaces.sort_by_key(|w| -(w.last_open() as i64));
        let workspaces = serde_json::to_string_pretty(&workspaces)?;
        std::fs::write(self.folder.join(RECENT_WORKSPACES), workspaces)?;

        Ok(())
    }

    pub fn save_window_tab(
        &self,
        data: WindowWorkspaceData,
        requester: &LocalTaskRequester,
    ) {
        let workspace = data.workspace.clone();
        let workspace_info = data.workspace_info();

        requester.notification(LocalNotification::DbSaveEvent(
            SaveEvent::Workspace(workspace, workspace_info),
        ));
    }

    pub fn get_workspace_info(
        &self,
        workspace: &LapceWorkspace,
    ) -> Result<WorkspaceInfo> {
        let info = std::fs::read_to_string(
            self.workspace_folder
                .join(workspace_folder_name(workspace))
                .join(WORKSPACE_INFO),
        )?;
        let info: WorkspaceInfo = serde_json::from_str(&info)?;
        Ok(info)
    }

    pub(crate) fn insert_workspace(
        &self,
        workspace: &LapceWorkspace,
        info: &WorkspaceInfo,
    ) -> Result<()> {
        let folder = self.workspace_folder.join(workspace_folder_name(workspace));
        if let Err(err) = std::fs::create_dir_all(&folder) {
            log::error!("{:?}", err);
        }
        let workspace_info = serde_json::to_string_pretty(info)?;
        std::fs::write(folder.join(WORKSPACE_INFO), workspace_info)?;
        Ok(())
    }

    pub fn save_app(&self, data: &AppData) {
        let windows = data.windows.get_untracked();
        for window in windows.values() {
            self.save_window(window.clone());
        }

        let info = AppInfo {
            windows: windows
                .values()
                .map(|window_data| window_data.info())
                .collect(),
        };
        if info.windows.is_empty() {
            return;
        }

        data.local_task
            .notification(LocalNotification::DbSaveEvent(SaveEvent::App(info)));
    }

    pub fn insert_app_info(&self, info: AppInfo) -> Result<()> {
        let info = serde_json::to_string_pretty(&info)?;
        std::fs::write(self.folder.join(APP), info)?;
        Ok(())
    }

    pub fn insert_app(&self, data: AppData) -> Result<()> {
        let windows = data.windows.get_untracked();
        if windows.is_empty() {
            // insert_app is called after window is closed, so we don't want to store
            // it
            return Ok(());
        }
        for window in windows.values() {
            if let Err(err) = self.insert_window(window.clone()) {
                log::error!("{:?}", err);
            }
        }
        let info = AppInfo {
            windows: windows
                .values()
                .map(|window_data| window_data.info())
                .collect(),
        };
        self.insert_app_info(info)?;
        Ok(())
    }

    pub fn get_app(&self) -> Result<AppInfo> {
        let info = std::fs::read_to_string(self.folder.join(APP))?;
        let mut info: AppInfo = serde_json::from_str(&info)?;
        for window in info.windows.iter_mut() {
            if window.size.width < 10.0 {
                window.size.width = 800.0;
            }
            if window.size.height < 10.0 {
                window.size.width = 600.0;
            }
        }
        Ok(info)
    }

    pub fn get_window(&self) -> Result<WindowInfo> {
        let info = std::fs::read_to_string(self.folder.join(WINDOW))?;
        let mut info: WindowInfo = serde_json::from_str(&info)?;
        if info.size.width < 10.0 {
            info.size.width = 800.0;
        }
        if info.size.height < 10.0 {
            info.size.width = 600.0;
        }
        Ok(info)
    }

    pub fn save_window(&self, data: WindowData) {
        self.save_window_tab(data.window_tabs.get_untracked(), &data.local_task);
    }

    pub fn insert_window(&self, data: WindowData) -> Result<()> {
        if let Err(err) = self.insert_window_tab(data.window_tabs.get_untracked()) {
            log::error!("{:?}", err);
        }
        let info = data.info();
        let info = serde_json::to_string_pretty(&info)?;
        std::fs::write(self.folder.join(WINDOW), info)?;
        Ok(())
    }

    pub fn insert_window_tab(&self, data: WindowWorkspaceData) -> Result<()> {
        let workspace = data.workspace.clone();
        let workspace_info = data.workspace_info();

        self.insert_workspace(&workspace, &workspace_info)?;
        // self.insert_unsaved_buffer(main_split)?;

        Ok(())
    }

    pub fn get_panel_orders(&self) -> Result<PanelOrder> {
        let panel_orders = std::fs::read_to_string(self.folder.join(PANEL_ORDERS))?;
        let mut panel_orders: PanelOrder = serde_json::from_str(&panel_orders)?;

        use strum::IntoEnumIterator;
        for kind in PanelKind::iter() {
            if kind.position(&panel_orders).is_none() {
                let panels =
                    panel_orders.entry(kind.default_position()).or_default();
                panels.push_back(kind);
            }
        }

        Ok(panel_orders)
    }

    pub fn save_panel_orders(
        &self,
        order: PanelOrder,
        requester: &LocalTaskRequester,
    ) {
        requester.notification(LocalNotification::DbSaveEvent(
            SaveEvent::PanelOrder(order),
        ));
    }

    pub(crate) fn insert_panel_orders(&self, order: &PanelOrder) -> Result<()> {
        let info = serde_json::to_string_pretty(order)?;
        std::fs::write(self.folder.join(PANEL_ORDERS), info)?;
        Ok(())
    }

    pub fn save_doc_position(
        &self,
        workspace: &LapceWorkspace,
        path: PathBuf,
        cursor_offset: usize,
        scroll_offset: Vec2,
        requester: &LocalTaskRequester,
    ) {
        let info = DocInfo {
            workspace: workspace.clone(),
            path,
            scroll_offset: (scroll_offset.x, scroll_offset.y),
            cursor_offset,
        };
        requester.notification(LocalNotification::DbSaveEvent(SaveEvent::Doc(info)));
    }

    pub(crate) fn insert_doc(&self, info: &DocInfo) -> Result<()> {
        let folder = self
            .workspace_folder
            .join(workspace_folder_name(&info.workspace))
            .join(WORKSPACE_FILES);
        if let Err(err) = std::fs::create_dir_all(&folder) {
            log::error!("{:?}", err);
        }
        let contents = serde_json::to_string_pretty(info)?;
        std::fs::write(folder.join(doc_path_name(&info.path)), contents)?;
        Ok(())
    }

    pub fn get_doc_info(
        &self,
        workspace: &LapceWorkspace,
        path: &Path,
    ) -> Result<DocInfo> {
        let folder = self
            .workspace_folder
            .join(workspace_folder_name(workspace))
            .join(WORKSPACE_FILES);
        let info = std::fs::read_to_string(folder.join(doc_path_name(path)))?;
        let info: DocInfo = serde_json::from_str(&info)?;
        Ok(info)
    }
}

fn workspace_folder_name(workspace: &LapceWorkspace) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_key_only(&workspace.to_string())
        .finish()
}

fn doc_path_name(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}
