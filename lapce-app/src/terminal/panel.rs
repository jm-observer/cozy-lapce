use anyhow::anyhow;
use std::{collections::HashMap, path::PathBuf, rc::Rc, sync::Arc};

use floem::views::editor::core::mode::Mode;
use floem::{
    ext_event::create_ext_action,
    reactive::{Memo, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
};
use lapce_rpc::{
    dap_types::{
        self, DapId, RunDebugConfig, StackFrame, Stopped, ThreadId, Variable,
    },
    proxy::ProxyResponse,
    terminal::{TermId, TerminalProfile},
};
use log::{debug, error};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::{data::TerminalData, tab::TerminalTabData};
use crate::terminal::event::TermEvent;
use crate::{
    debug::{
        DapData, DapVariable, RunDebugConfigs, RunDebugData, RunDebugMode,
        RunDebugProcess, ScopeOrVar,
    },
    id::TerminalTabId,
    keypress::{EventRef, KeyPressData, KeyPressFocus, KeyPressHandle},
    main_split::MainSplitData,
    panel::kind::PanelKind,
    terminal::raw::RawTerminal,
    window_tab::{CommonData, Focus},
    workspace::LapceWorkspace,
};

pub struct TerminalTabInfo {
    pub active: Option<TerminalTabId>,
    pub tabs: im::Vector<TerminalTabData>,
}

impl TerminalTabInfo {
    pub fn active_tab(&self) -> Option<(usize, &TerminalTabData)> {
        self.active.and_then(|active| {
            self.tabs
                .iter()
                .enumerate()
                .find(|(_index, tab)| tab.terminal_tab_id == active)
        })
    }

    pub fn next_tab(&mut self) {
        let mut active_index = self.active_tab().map(|x| x.0).unwrap_or_default();
        if active_index >= self.tabs.len().saturating_sub(1) {
            active_index = 0;
        } else {
            active_index += 1;
        }
        self.active = self.tabs.get(active_index).map(|x| x.terminal_tab_id);
    }

    pub fn previous_tab(&mut self) {
        let mut active_index = self.active_tab().map(|x| x.0).unwrap_or_default();
        if active_index == 0 {
            active_index = self.tabs.len().saturating_sub(1);
        } else {
            active_index -= 1;
        }
        self.active = self.tabs.get(active_index).map(|x| x.terminal_tab_id);
    }
}

#[derive(Clone)]
pub struct TerminalPanelData {
    pub cx: Scope,
    pub workspace: Arc<LapceWorkspace>,
    pub tab_infos: RwSignal<TerminalTabInfo>,
    pub debug: RunDebugData,
    pub breakline: Memo<Option<(usize, PathBuf)>>,
    pub common: Rc<CommonData>,
    pub main_split: MainSplitData,
}

impl TerminalPanelData {
    pub fn new(
        workspace: Arc<LapceWorkspace>,
        profile: Option<TerminalProfile>,
        common: Rc<CommonData>,
        main_split: MainSplitData,
    ) -> Self {
        let terminal_tab =
            TerminalTabData::new(workspace.clone(), profile, common.clone());

        let cx = common.scope;
        let active = Some(terminal_tab.terminal_tab_id);
        let tabs = im::vector![terminal_tab];
        let tab_info = TerminalTabInfo { active, tabs };
        let tab_info = cx.create_rw_signal(tab_info);

        let debug = RunDebugData::new(cx, common.breakpoints);

        let breakline = {
            let active_term = debug.active_term;
            let daps = debug.daps;
            cx.create_memo(move |_| {
                let active_term = active_term.get();
                let active_term = match active_term {
                    Some(active_term) => active_term,
                    None => return None,
                };

                let term = tab_info.with_untracked(|info| {
                    for tab in &info.tabs {
                        let terminal = tab.terminal.with_untracked(|terminals| {
                            if terminals.term_id == active_term {
                                Some(terminals.clone())
                            } else {
                                None
                            }
                        });
                        if let Some(terminal) = terminal {
                            return Some(terminal);
                        }
                    }
                    None
                });
                let term = match term {
                    Some(term) => term,
                    None => return None,
                };
                let stopped = term
                    .run_debug
                    .with(|run_debug| run_debug.as_ref().map(|r| r.stopped))
                    .unwrap_or(true);
                if stopped {
                    return None;
                }

                let daps = daps.get();
                let dap = daps.values().find(|d| d.term_id == Some(active_term));
                dap.and_then(|dap| dap.breakline.get())
            })
        };

        Self {
            cx,
            workspace,
            tab_infos: tab_info,
            debug,
            breakline,
            common,
            main_split,
        }
    }

    pub fn active_tab(&self, tracked: bool) -> Option<TerminalTabData> {
        if tracked {
            self.tab_infos.with(|info| {
                info.active_tab().map(|x| x.1.clone())
                // info.tabs
                //     .get(info.active)
                //     .or_else(|| info.tabs.last())
                //     .cloned()
                //     .map(|(_, tab)| tab)
            })
        } else {
            self.tab_infos
                .with_untracked(|info| info.active_tab().map(|x| x.1.clone()))
        }
    }

    pub fn key_down<'a>(
        &self,
        event: impl Into<EventRef<'a>> + Copy,
        keypress: &KeyPressData,
    ) -> Option<KeyPressHandle> {
        if self.tab_infos.with_untracked(|info| info.tabs.is_empty()) {
            self.new_tab(None);
        }

        let tab = self.active_tab(false);
        let terminal = tab.map(|tab| tab.active_terminal(false));
        if let Some(terminal) = terminal {
            let handle = keypress.key_down(event, &terminal);
            let mode = terminal.get_mode();

            if !handle.handled && mode == Mode::Terminal {
                if let EventRef::Keyboard(key_event) = event.into() {
                    if terminal.send_keypress(key_event) {
                        return Some(KeyPressHandle {
                            handled: true,
                            keymatch: handle.keymatch,
                            keypress: handle.keypress,
                        });
                    }
                }
            }
            Some(handle)
        } else {
            None
        }
    }

    pub fn new_tab(&self, profile: Option<TerminalProfile>) {
        self.new_tab_run_debug(None, profile);
    }

    /// Create a new terminal tab with the given run debug process.  
    /// Errors if expanding out the run debug process failed.
    pub fn new_tab_run_debug(
        &self,
        run_debug: Option<RunDebugProcess>,
        profile: Option<TerminalProfile>,
    ) -> TerminalTabData {
        let terminal_tab = TerminalTabData::new_run_debug(
            self.workspace.clone(),
            run_debug,
            profile,
            self.common.clone(),
        );
        let tab_id = terminal_tab.terminal_tab_id;
        let update_terminal = terminal_tab.clone();
        self.tab_infos.update(|info| {
            info.tabs.push_back(update_terminal);
            info.active = Some(tab_id);
        });

        terminal_tab
    }

    pub fn next_tab(&self) {
        self.tab_infos.update(|info| {
            info.next_tab();
        });
        self.update_debug_active_term();
    }

    pub fn previous_tab(&self) {
        self.tab_infos.update(|info| {
            info.previous_tab();
        });
        self.update_debug_active_term();
    }

    // todo why option?
    pub fn close_tab(&self, terminal_tab_id: Option<TerminalTabId>) {
        if let Some(close_tab) = self
            .tab_infos
            .try_update(|info| {
                let mut close_tab = None;
                if let Some(terminal_tab_id) = terminal_tab_id {
                    if let Some(index) =
                        info.tabs.iter().enumerate().find_map(|(index, t)| {
                            if t.terminal_tab_id == terminal_tab_id {
                                Some(index)
                            } else {
                                None
                            }
                        })
                    {
                        close_tab =
                            Some(info.tabs.remove(index).terminal.get_untracked());
                    }
                    // } else {
                    //     let mut active_index =
                    //         info.active_tab().map(|x| x.0).unwrap_or_default();
                    //     let active = info.active.min(info.tabs.len().saturating_sub(1));
                    //     if !info.tabs.is_empty() {
                    //         info.tabs.remove(active);
                    //     }
                }
                if info.active == terminal_tab_id {
                    info.next_tab();
                }
                close_tab
            })
            .flatten()
        {
            close_tab.stop();
        }
        self.update_debug_active_term();
    }

    pub fn set_title(&self, term_id: &TermId, title: &str) {
        if let Some(t) = self.get_terminal(*term_id) {
            t.title.set(title.to_string());
        }
    }

    pub fn get_tab_terminal(&self, term_id: TermId) -> Option<TerminalTabData> {
        self.tab_infos.with_untracked(|info| {
            for tab in &info.tabs {
                if tab.terminal_tab_id == term_id {
                    return Some(tab.clone());
                }
            }
            None
        })
    }

    pub fn get_terminal(&self, term_id: TermId) -> Option<TerminalData> {
        let tab = self.get_tab_terminal(term_id)?;
        Some(tab.terminal.get_untracked())
    }

    fn get_terminal_in_tab(
        &self,
        term_id: &TermId,
    ) -> Option<(TerminalTabData, TerminalData)> {
        self.tab_infos.with_untracked(|info| {
            for tab in info.tabs.iter() {
                let result = tab.terminal.with_untracked(|terminals| {
                    if terminals.term_id == *term_id {
                        Some(terminals.clone())
                    } else {
                        None
                    }
                });
                if let Some(terminal) = result {
                    return Some((tab.clone(), terminal));
                }
            }
            None
        })
    }

    // pub fn split(&self, term_id: TermId) {
    //     if let Some((_, tab, index, _)) = self.get_terminal_in_tab(&term_id) {
    //         let terminal_data = TerminalData::new(
    //             tab.scope,
    //             self.workspace.clone(),
    //             None,
    //             self.common.clone(),
    //         );
    //         let i = terminal_data.scope.create_rw_signal(0);
    //         tab.terminal.update(|terminals| {
    //             terminals.insert(index + 1, (i, terminal_data));
    //         });
    //     }
    // }

    // pub fn split_next(&self, term_id: TermId) {
    //     if let Some((_, tab, index, _)) = self.get_terminal_in_tab(&term_id) {
    //         let max = tab.terminal.with_untracked(|t| t.len() - 1);
    //         let new_index = (index + 1).min(max);
    //         if new_index != index {
    //             tab.active.set(new_index);
    //             self.update_debug_active_term();
    //         }
    //     }
    // }

    // pub fn split_previous(&self, term_id: TermId) {
    //     if let Some((_, tab, index, _)) = self.get_terminal_in_tab(&term_id) {
    //         let new_index = index.saturating_sub(1);
    //         if new_index != index {
    //             tab.active.set(new_index);
    //             self.update_debug_active_term();
    //         }
    //     }
    // }
    //
    // pub fn split_exchange(&self, term_id: TermId) {
    //     if let Some((_, tab, index, _)) = self.get_terminal_in_tab(&term_id) {
    //         let max = tab.terminal.with_untracked(|t| t.len() - 1);
    //         if index < max {
    //             tab.terminal.update(|terminals| {
    //                 terminals.swap(index, index + 1);
    //             });
    //             self.update_debug_active_term();
    //         }
    //     }
    // }

    pub fn close_terminal(&self, term_id: &TermId) {
        // todo close tab directly
        if let Some((tab, _terminal_data)) = self.get_terminal_in_tab(term_id) {
            self.close_tab(Some(tab.terminal_tab_id));
        }
    }

    pub fn launch_failed(&self, term_id: &TermId, error: &str) {
        if let Some(terminal) = self.get_terminal(*term_id) {
            terminal.launch_error.set(Some(error.to_string()));
        }
    }

    pub fn terminal_stopped(&self, term_id: &TermId, exit_code: Option<i32>) {
        if let Some(terminal) = self.get_terminal(*term_id) {
            if terminal.run_debug.with_untracked(|r| r.is_some()) {
                let (was_prelaunch, dap_id) = terminal
                    .run_debug
                    .try_update(|run_debug| {
                        if let Some(run_debug) = run_debug.as_mut() {
                            let dap_id = run_debug.origin_config.dap_id;
                            if run_debug.is_prelaunch
                                && run_debug.config.prelaunch.is_some()
                            {
                                run_debug.is_prelaunch = false;
                                if run_debug.mode == RunDebugMode::Debug {
                                    // set it to be stopped so that the dap can pick the same terminal session
                                    run_debug.stopped = true;
                                }
                                Some((true, dap_id))
                            } else {
                                run_debug.stopped = true;
                                Some((false, dap_id))
                            }
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .unwrap();
                let exit_code = exit_code.unwrap_or(0);
                if was_prelaunch && exit_code == 0 {
                    self.debug.daps.try_update(|x| {
                        if let Some(process) = x.get_mut(&dap_id) {
                            process.term_id = Some(*term_id);
                        }
                    });
                    let run_debug = terminal.run_debug.get_untracked();
                    if let Some(mut run_debug) = run_debug {
                        if run_debug.mode == RunDebugMode::Debug {
                            update_executable(
                                &mut run_debug,
                                terminal.raw.get_untracked().clone(),
                            );
                            self.common.proxy.dap_start(
                                run_debug.config,
                                self.debug.source_breakpoints(),
                            )
                        } else {
                            terminal.new_process(Some(run_debug));
                        }
                    }
                }
            } else {
                todo!("???")
                // self.close_terminal(term_id);
            }
        }
    }

    pub fn get_stopped_run_debug_terminal(
        &self,
        mode: &RunDebugMode,
        config: &RunDebugConfig,
    ) -> Option<TerminalData> {
        self.tab_infos.with_untracked(|info| {
            for tab in &info.tabs {
                let terminal = tab.terminal.with_untracked(|terminal| {
                    if let Some(run_debug) =
                        terminal.run_debug.get_untracked().as_ref()
                    {
                        if run_debug.stopped && &run_debug.mode == mode {
                            match run_debug.mode {
                                RunDebugMode::Run => {
                                    if run_debug.config.name == config.name {
                                        return Some(terminal.clone());
                                    }
                                }
                                RunDebugMode::Debug => {
                                    if run_debug.config.dap_id == config.dap_id {
                                        return Some(terminal.clone());
                                    }
                                }
                            }
                        }
                    }
                    None
                });
                if let Some(terminal) = terminal {
                    return Some(terminal);
                }
            }
            None
        })
    }

    /// Return whether it is in debug mode.
    pub fn restart_run_debug(&self, term_id: TermId) -> Option<bool> {
        let (terminal_tab, terminal) = self.get_terminal_in_tab(&term_id)?;
        let mut run_debug = terminal.run_debug.get_untracked()?;
        if run_debug.config.config_source.from_palette() {
            if let Some(new_config) =
                self.get_run_config_by_name(&run_debug.config.name)
            {
                run_debug.config = new_config;
            }
        }
        let mut is_debug = false;
        let new_term_id = match run_debug.mode {
            RunDebugMode::Run => {
                self.common
                    .proxy
                    .terminal_close(terminal.term_id, terminal.raw_id());
                let mut run_debug = run_debug;
                run_debug.stopped = false;
                run_debug.is_prelaunch = true;
                let new_terminal = TerminalData::new_run_debug(
                    terminal_tab.scope,
                    self.workspace.clone(),
                    Some(run_debug),
                    None,
                    self.common.clone(),
                );
                let new_term_id = new_terminal.term_id;
                terminal_tab.terminal.update(|terminals| {
                    *terminals = new_terminal;
                });
                self.debug.active_term.set(Some(new_term_id));
                new_term_id
            }
            RunDebugMode::Debug => {
                is_debug = true;
                let config = terminal.run_debug.get_untracked()?.config;
                // let daps = self.debug.daps.get_untracked();
                self.common
                    .proxy
                    .dap_restart(config, self.debug.source_breakpoints());
                term_id
            }
        };

        self.focus_terminal(new_term_id);

        Some(is_debug)
    }

    fn get_run_config_by_name(&self, name: &str) -> Option<RunDebugConfig> {
        if let Some(workspace) = self.common.workspace.path.as_deref() {
            let run_toml = workspace.join(".lapce").join("run.toml");
            let (doc, new_doc) =
                self.main_split.get_doc(run_toml.clone(), None, false);
            if !new_doc {
                let content = doc.lines.with_untracked(|x| x.buffer().to_string());
                match toml::from_str::<RunDebugConfigs>(&content) {
                    Ok(configs) => {
                        return configs.configs.into_iter().find(|x| x.name == name);
                    }
                    Err(err) => {
                        // todo show message window
                        log::error!("deser fail {:?}", err);
                    }
                }
            }
        }
        None
    }

    pub fn focus_terminal(&self, terminal_id: TerminalTabId) {
        self.tab_infos.update(|info| {
            info.active = Some(terminal_id);
        });
        self.common.focus.set(Focus::Panel(PanelKind::Terminal));

        self.update_debug_active_term();
    }

    pub fn update_debug_active_term(&self) {
        let tab = self.active_tab(false);
        let terminal = tab.map(|tab| tab.active_terminal(false));
        if let Some(terminal) = terminal {
            let term_id = terminal.term_id;
            let is_run_debug =
                terminal.run_debug.with_untracked(|run| run.is_some());
            let current_active = self.debug.active_term.get_untracked();
            if is_run_debug {
                if current_active != Some(term_id) {
                    self.debug.active_term.set(Some(term_id));
                }
            } else if let Some(active) = current_active {
                if self.get_terminal(active).is_none() {
                    self.debug.active_term.set(None);
                }
            }
        } else {
            self.debug.active_term.set(None);
        }
    }

    pub fn manual_stop_run_debug(&self, terminal_id: TerminalTabId) {
        if let Err(err) = self._manual_stop_run_debug(terminal_id) {
            error!("manual_stop_run_debug {:?}", err);
        }
    }
    fn _manual_stop_run_debug(
        &self,
        terminal_id: TerminalTabId,
    ) -> anyhow::Result<()> {
        let terminal = self
            .get_terminal(terminal_id)
            .ok_or(anyhow!("not found terminal data {terminal_id:?}"))?;
        let Some(run_debug) = terminal
            .run_debug
            .try_update(|x| {
                let mut stopped = false;
                if let Some(x) = x.as_mut() {
                    stopped = x.stopped;
                    x.stopped = true
                }
                if stopped {
                    None
                } else {
                    x.clone()
                }
            })
            .flatten()
        else {
            return Ok(());
        };

        debug!(
            "manual_stop_run_debug {:?} {:?}",
            run_debug.mode, terminal.term_id
        );
        match run_debug.mode {
            RunDebugMode::Run => {
                self.common
                    .proxy
                    .terminal_close(terminal.term_id, terminal.raw_id());
                self.common
                    .term_tx
                    .send((terminal.term_id, TermEvent::CloseTerminal))?;
            }
            RunDebugMode::Debug => {
                let dap_id = run_debug.config.dap_id;
                self.debug
                    .daps
                    .try_update(|x| x.remove(&dap_id))
                    .flatten()
                    .ok_or(anyhow!("not found dap data {dap_id:?}"))?;

                self.common.proxy.dap_stop(dap_id);
                // terminal close by dap
                // self.common
                //     .term_tx
                //     .send((terminal.term_id, TermEvent::CloseTerminal))?;
            }
        }
        self.focus_terminal(terminal_id);
        Ok(())
    }

    pub fn run_debug_process(
        &self,
        tracked: bool,
    ) -> Vec<(TermId, RunDebugProcess)> {
        let mut processes = Vec::new();
        if tracked {
            self.tab_infos.with(|info| {
                for tab in &info.tabs {
                    tab.terminal.with(|terminal| {
                        if let Some(run_debug) = terminal.run_debug.get() {
                            processes.push((terminal.term_id, run_debug));
                        }
                    })
                }
            });
        } else {
            self.tab_infos.with_untracked(|info| {
                for tab in &info.tabs {
                    tab.terminal.with_untracked(|terminal| {
                        if let Some(run_debug) = terminal.run_debug.get() {
                            processes.push((terminal.term_id, run_debug));
                        }
                    })
                }
            });
        }
        processes.sort_by_key(|(_, process)| process.created);
        processes
    }

    pub fn set_process_id(&self, term_id: &TermId, process_id: Option<u32>) {
        if let Some(terminal) = self.get_terminal(*term_id) {
            terminal.run_debug.with_untracked(|run_debug| {
                if let Some(run_debug) = run_debug.as_ref() {
                    if run_debug.config.debug_command.is_some() {
                        let dap_id = run_debug.config.dap_id;
                        self.common
                            .proxy
                            .dap_process_id(dap_id, process_id, *term_id);
                    }
                }
            });
        }
    }

    pub fn dap_continued(&self, dap_id: &DapId) {
        let dap = self
            .debug
            .daps
            .with_untracked(|daps| daps.get(dap_id).cloned());
        if let Some(dap) = dap {
            dap.thread_id.set(None);
            dap.stopped.set(false);
        }
    }

    pub fn dap_stopped(
        &self,
        dap_id: &DapId,
        stopped: &Stopped,
        stack_frames: &HashMap<ThreadId, Vec<StackFrame>>,
        variables: &[(dap_types::Scope, Vec<Variable>)],
    ) {
        let dap = self
            .debug
            .daps
            .with_untracked(|daps| daps.get(dap_id).cloned());
        if let Some(dap) = dap {
            dap.stopped(self.cx, stopped, stack_frames, variables);
        }
        floem::action::focus_window();
    }

    pub fn dap_continue(&self, term_id: TermId) -> Option<()> {
        let terminal = self.get_terminal(term_id)?;
        let dap_id = terminal
            .run_debug
            .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?;
        let thread_id = self.debug.daps.with_untracked(|daps| {
            daps.get(&dap_id)
                .and_then(|dap| dap.thread_id.get_untracked())
        });
        let thread_id = thread_id.unwrap_or_default();
        self.common.proxy.dap_continue(dap_id, thread_id);
        Some(())
    }

    pub fn dap_start(&self, config: RunDebugConfig) {
        self.common
            .proxy
            .dap_start(config, self.debug.source_breakpoints());
    }

    pub fn dap_pause(&self, term_id: TermId) -> Option<()> {
        let terminal = self.get_terminal(term_id)?;
        let dap_id = terminal
            .run_debug
            .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?;
        let thread_id = self.debug.daps.with_untracked(|daps| {
            daps.get(&dap_id)
                .and_then(|dap| dap.thread_id.get_untracked())
        });
        let thread_id = thread_id.unwrap_or_default();
        self.common.proxy.dap_pause(dap_id, thread_id);
        Some(())
    }

    pub fn dap_step_over(&self, term_id: TermId) -> Option<()> {
        let terminal = self.get_terminal(term_id)?;
        let dap_id = terminal
            .run_debug
            .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?;
        let thread_id = self.debug.daps.with_untracked(|daps| {
            daps.get(&dap_id)
                .and_then(|dap| dap.thread_id.get_untracked())
        });
        let thread_id = thread_id.unwrap_or_default();
        self.common.proxy.dap_step_over(dap_id, thread_id);
        Some(())
    }

    pub fn dap_step_into(&self, term_id: TermId) -> Option<()> {
        let terminal = self.get_terminal(term_id)?;
        let dap_id = terminal
            .run_debug
            .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?;
        let thread_id = self.debug.daps.with_untracked(|daps| {
            daps.get(&dap_id)
                .and_then(|dap| dap.thread_id.get_untracked())
        });
        let thread_id = thread_id.unwrap_or_default();
        self.common.proxy.dap_step_into(dap_id, thread_id);
        Some(())
    }

    pub fn dap_step_out(&self, term_id: TermId) -> Option<()> {
        let terminal = self.get_terminal(term_id)?;
        let dap_id = terminal
            .run_debug
            .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?;
        let thread_id = self.debug.daps.with_untracked(|daps| {
            daps.get(&dap_id)
                .and_then(|dap| dap.thread_id.get_untracked())
        });
        let thread_id = thread_id.unwrap_or_default();
        self.common.proxy.dap_step_out(dap_id, thread_id);
        Some(())
    }

    pub fn get_active_dap(&self, tracked: bool) -> Option<DapData> {
        let active_term = if tracked {
            self.debug.active_term.get()?
        } else {
            self.debug.active_term.get_untracked()?
        };
        self.get_dap(active_term, tracked)
    }

    pub fn get_dap(
        &self,
        terminal_tab_id: TerminalTabId,
        tracked: bool,
    ) -> Option<DapData> {
        let terminal = self.get_terminal(terminal_tab_id)?;
        let dap_id = if tracked {
            terminal
                .run_debug
                .with(|r| r.as_ref().map(|r| r.config.dap_id))?
        } else {
            terminal
                .run_debug
                .with_untracked(|r| r.as_ref().map(|r| r.config.dap_id))?
        };

        if tracked {
            self.debug.daps.with(|daps| daps.get(&dap_id).cloned())
        } else {
            self.debug
                .daps
                .with_untracked(|daps| daps.get(&dap_id).cloned())
        }
    }

    pub fn dap_frame_scopes(&self, dap_id: DapId, frame_id: usize) {
        if let Some(dap) = self.debug.daps.get_untracked().get(&dap_id) {
            let variables = dap.variables;
            let send = create_ext_action(self.common.scope, move |result| {
                if let Ok(ProxyResponse::DapGetScopesResponse { scopes }) = result {
                    variables.update(|dap_var| {
                        dap_var.children = scopes
                            .iter()
                            .enumerate()
                            .map(|(i, (scope, vars))| DapVariable {
                                item: ScopeOrVar::Scope(scope.to_owned()),
                                parent: Vec::new(),
                                expanded: i == 0,
                                read: i == 0,
                                children: vars
                                    .iter()
                                    .map(|var| DapVariable {
                                        item: ScopeOrVar::Var(var.to_owned()),
                                        parent: vec![scope.variables_reference],
                                        expanded: false,
                                        read: false,
                                        children: Vec::new(),
                                        children_expanded_count: 0,
                                    })
                                    .collect(),
                                children_expanded_count: if i == 0 {
                                    vars.len()
                                } else {
                                    0
                                },
                            })
                            .collect();
                        dap_var.children_expanded_count = dap_var
                            .children
                            .iter()
                            .map(|v| v.children_expanded_count + 1)
                            .sum::<usize>();
                    });
                }
            });

            self.common.proxy.dap_get_scopes(
                dap_id,
                frame_id,
                move |(_, result)| {
                    send(result);
                },
            );
        }
    }
}

fn update_executable(
    run_debug: &mut RunDebugProcess,
    raw: Arc<RwLock<RawTerminal>>,
) {
    if run_debug.config.config_source.from_rust_code_lens() {
        let lines = raw.write_arc().output(5);
        if let Some(executable) = lines.into_iter().rev().find_map(|x| {
            if let Ok(map) = serde_json::from_str::<RustArtifact>(&x) {
                return map.artifact();
            } else {
                log::debug!("{}", x);
            }
            None
        }) {
            run_debug.config.program = executable;
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Profile {
    pub test: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct Target {
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RustArtifact {
    pub reason: String,
    pub target: Target,
    pub profile: Profile,
    pub executable: String,
}

impl RustArtifact {
    pub fn artifact(self) -> Option<String> {
        if &self.reason == "compiler-artifact" && !self.executable.is_empty() {
            let is_binary = self.target.kind.contains(&"bin".to_owned());
            let is_build_script =
                self.target.crate_types.contains(&"custom-build".to_owned());
            if (is_binary && !is_build_script) || self.profile.test {
                return Some(self.executable);
            }
        }
        None
    }
}
