use std::{rc::Rc, sync::Arc};

use floem::reactive::{RwSignal, Scope, SignalWith};
use lapce_rpc::terminal::TerminalProfile;

use super::data::TerminalData;
use crate::{
    debug::RunDebugProcess, id::TerminalTabId, window_tab::CommonData,
    workspace::LapceWorkspace,
};

#[derive(Clone)]
pub struct TerminalTabData {
    pub scope: Scope,
    pub terminal_tab_id: TerminalTabId,
    pub terminal: RwSignal<TerminalData>,
}

impl TerminalTabData {
    pub fn new(
        workspace: Arc<LapceWorkspace>,
        profile: Option<TerminalProfile>,
        common: Rc<CommonData>,
    ) -> Self {
        TerminalTabData::new_run_debug(workspace, None, profile, common)
    }

    /// Create the information for a terminal tab, which can contain multiple terminals.  
    pub fn new_run_debug(
        workspace: Arc<LapceWorkspace>,
        run_debug: Option<RunDebugProcess>,
        profile: Option<TerminalProfile>,
        common: Rc<CommonData>,
    ) -> Self {
        let cx = common.scope.create_child();
        let terminal_data =
            TerminalData::new_run_debug(cx, workspace, run_debug, profile, common);
        let terminal_tab_id = terminal_data.term_id;
        let terminals = cx.create_rw_signal(terminal_data);
        Self {
            scope: cx,
            terminal_tab_id,
            terminal: terminals,
        }
    }

    pub fn active_terminal(&self, tracked: bool) -> TerminalData {
        if tracked {
            self.terminal.with(|t| t.clone())
        } else {
            self.terminal.with_untracked(|t| t.clone())
        }
    }
}
