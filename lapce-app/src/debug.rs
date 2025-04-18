use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

use doc::lines::{DocLinesManager, buffer::rope_text::RopeText};
use floem::{
    ext_event::create_ext_action,
    reactive::{Memo, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
    views::VirtualVector,
};
use lapce_core::debug::{
    BreakpointAction, DapVariableViewdata, LapceBreakpoint, ScopeOrVar,
};
use lapce_rpc::{
    dap_types::{
        self, DapId, SourceBreakpoint, StackFrame, Stopped, ThreadId, Variable,
    },
    proxy::{ProxyResponse, ProxyRpcHandler},
    terminal::TermId,
};
use lapce_xi_rope::{Rope, RopeDelta, Transformer};
use log::error;

use crate::{
    command::InternalCommand,
    editor::location::{EditorLocation, EditorPosition},
    window_workspace::CommonData,
};

#[derive(Clone, Copy)]
pub struct BreakPoints {
    pub breakpoints: RwSignal<BTreeMap<PathBuf, BTreeMap<usize, LapceBreakpoint>>>,
}

impl BreakPoints {
    pub fn clone_for_hashmap(&self) -> HashMap<PathBuf, Vec<LapceBreakpoint>> {
        self.breakpoints
            .get_untracked()
            .into_iter()
            .map(|(path, breakpoints)| {
                (path, breakpoints.into_values().collect::<Vec<_>>())
            })
            .collect()
    }

    pub fn set(
        &self,
        breakpoints: BTreeMap<PathBuf, BTreeMap<usize, LapceBreakpoint>>,
    ) {
        self.breakpoints.set(breakpoints);
    }

    pub fn add_or_remove_by_path_line_offset(
        &self,
        path: &Path,
        line: usize,
        offset: usize,
    ) -> BTreeMap<usize, LapceBreakpoint> {
        self.breakpoints
            .try_update(|breakpoints| {
                let breakpoints = breakpoints.entry(path.to_path_buf()).or_default();
                if let std::collections::btree_map::Entry::Vacant(e) =
                    breakpoints.entry(line)
                {
                    e.insert(LapceBreakpoint {
                        id: None,
                        verified: false,
                        message: None,
                        line,
                        offset,
                        dap_line: None,
                        active: true,
                    });
                } else {
                    breakpoints.remove(&line);
                }
                breakpoints.clone()
            })
            .unwrap()
    }

    pub fn add_by_path_line_offset(
        &self,
        path: &Path,
        line: usize,
        offset: usize,
    ) -> BTreeMap<usize, LapceBreakpoint> {
        self.breakpoints
            .try_update(|breakpoints| {
                let breakpoints = breakpoints.entry(path.to_path_buf()).or_default();
                if let std::collections::btree_map::Entry::Vacant(e) =
                    breakpoints.entry(line)
                {
                    e.insert(LapceBreakpoint {
                        id: None,
                        verified: false,
                        message: None,
                        line,
                        offset,
                        dap_line: None,
                        active: true,
                    });
                } else {
                    let mut toggle_active = false;
                    if let Some(breakpint) = breakpoints.get_mut(&line) {
                        if !breakpint.active {
                            breakpint.active = true;
                            toggle_active = true;
                        }
                    }
                    if !toggle_active {
                        breakpoints.remove(&line);
                    }
                }
                breakpoints.clone()
            })
            .unwrap()
    }

    pub fn toggle_by_path_line(
        &self,
        path: &Path,
        line: usize,
    ) -> BTreeMap<usize, LapceBreakpoint> {
        self.breakpoints
            .try_update(|breakpoints| {
                let breakpoints = breakpoints.entry(path.to_path_buf()).or_default();
                if let Some(breakpint) = breakpoints.get_mut(&line) {
                    breakpint.active = !breakpint.active;
                    breakpint.verified = false;
                }
                breakpoints.clone()
            })
            .unwrap()
    }

    pub fn remove_by_path_line(
        &self,
        path: &Path,
        line: usize,
    ) -> BTreeMap<usize, LapceBreakpoint> {
        self.breakpoints
            .try_update(|breakpoints| {
                let path_breakpoints =
                    breakpoints.entry(path.to_path_buf()).or_default();
                path_breakpoints.remove(&line);
                if path_breakpoints.is_empty() {
                    breakpoints.remove(path);
                    BTreeMap::new()
                } else {
                    path_breakpoints.clone()
                }
            })
            .unwrap()
    }

    pub fn get_by_path_tracked(
        &self,
        path: &Path,
    ) -> BTreeMap<usize, LapceBreakpoint> {
        self.breakpoints
            .with(|x| x.get(path).cloned().unwrap_or_default())
    }

    pub fn update_by_dap_resp(
        &self,
        path: &PathBuf,
        breakpoints: &Vec<dap_types::Breakpoint>,
    ) {
        self.breakpoints.update(|all_breakpoints| {
            if let Some(current_breakpoints) = all_breakpoints.get_mut(path) {
                let mut line_changed = HashSet::new();
                let mut i = 0;
                for (_, current_breakpoint) in current_breakpoints.iter_mut() {
                    if !current_breakpoint.active {
                        continue;
                    }
                    if let Some(breakpoint) = breakpoints.get(i) {
                        current_breakpoint.id = breakpoint.id;
                        current_breakpoint.verified = breakpoint.verified;
                        current_breakpoint.message.clone_from(&breakpoint.message);
                        if let Some(new_line) = breakpoint.line {
                            if current_breakpoint.line + 1 != new_line {
                                line_changed.insert(current_breakpoint.line);
                                current_breakpoint.line = new_line.saturating_sub(1);
                            }
                        }
                    }
                    i += 1;
                }
                for line in line_changed {
                    if let Some(changed) = current_breakpoints.remove(&line) {
                        current_breakpoints.insert(changed.line, changed);
                    }
                }
            }
        });
    }

    pub fn update_by_rope_delta(
        &self,
        delta: &RopeDelta,
        path: &Path,
        old_text: &Rope,
        lines: DocLinesManager,
    ) {
        self.breakpoints.update(|breakpoints| {
            if let Some(path_breakpoints) = breakpoints.get_mut(path) {
                let mut transformer = Transformer::new(delta);
                lines.with_untracked(|buffer| {
                    let buffer = buffer.buffer();
                    *path_breakpoints = path_breakpoints
                        .clone()
                        .into_values()
                        .map(|mut b| {
                            let offset = old_text.offset_of_line(b.line)?;
                            let offset = transformer.transform(offset, false);
                            let line = buffer.line_of_offset(offset);
                            b.line = line;
                            b.offset = offset;
                            Ok((b.line, b))
                        })
                        .filter_map(|x: anyhow::Result<(usize, LapceBreakpoint)>| {
                            match x {
                                Ok(rs) => Some(rs),
                                Err(err) => {
                                    error!("{err:?}");
                                    None
                                },
                            }
                        })
                        .collect();
                });
            }
        })
    }

    pub fn update_by_stopped(&self) {
        self.breakpoints.update(|breakpoints| {
            breakpoints
                .iter_mut()
                .for_each(|x| x.1.iter_mut().for_each(|x| x.1.verified = false));
        })
    }

    pub fn source_breakpoints_untracked(
        &self,
    ) -> std::collections::HashMap<PathBuf, Vec<SourceBreakpoint>> {
        self.breakpoints
            .get_untracked()
            .iter()
            .map(|(path, breakpoints)| {
                (
                    path.to_path_buf(),
                    breakpoints
                        .iter()
                        .filter_map(|(_, b)| {
                            if b.active {
                                Some(SourceBreakpoint {
                                    line:          b.line + 1,
                                    column:        None,
                                    condition:     None,
                                    hit_condition: None,
                                    log_message:   None,
                                })
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        self.breakpoints.with_untracked(|x| x.contains_key(path))
    }

    pub fn view_data(&self) -> impl IntoIterator<Item = (PathBuf, LapceBreakpoint)> {
        self.breakpoints
            .get()
            .into_iter()
            .flat_map(|(path, breakpoints)| {
                breakpoints.into_values().map(move |b| (path.clone(), b))
            })
    }
}

#[derive(Clone)]
pub struct RunDebugData {
    pub active_term: RwSignal<Option<TermId>>,
    pub daps:        RwSignal<im::HashMap<DapId, DapData>>,
    pub breakpoints: BreakPoints,
}

impl RunDebugData {
    pub fn new(cx: Scope, breakpoints: BreakPoints) -> Self {
        let active_term: RwSignal<Option<TermId>> = cx.create_rw_signal(None);
        let daps: RwSignal<im::HashMap<DapId, DapData>> =
            cx.create_rw_signal(im::HashMap::new());

        Self {
            active_term,
            daps,
            breakpoints,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct StackTraceData {
    pub expanded:     RwSignal<bool>,
    pub frames:       RwSignal<im::Vector<StackFrame>>,
    pub frames_shown: usize,
}

#[derive(Clone, Default)]
pub struct DapVariable {
    pub item:                    ScopeOrVar,
    pub parent:                  Vec<usize>,
    pub expanded:                bool,
    pub read:                    bool,
    pub children:                Vec<DapVariable>,
    pub children_expanded_count: usize,
}

#[derive(Clone)]
pub struct DapData {
    pub term_id:      Option<TermId>,
    pub dap_id:       DapId,
    pub stopped:      RwSignal<bool>,
    pub thread_id:    RwSignal<Option<ThreadId>>,
    pub stack_traces: RwSignal<BTreeMap<ThreadId, StackTraceData>>,
    pub variables_id: RwSignal<usize>,
    pub variables:    RwSignal<DapVariable>,
    pub breakline:    Memo<Option<(usize, PathBuf)>>,
    pub common:       Rc<CommonData>,
}

impl DapData {
    pub fn new(
        cx: Scope,
        dap_id: DapId,
        term_id: Option<TermId>,
        common: Rc<CommonData>,
    ) -> Self {
        let stopped = cx.create_rw_signal(false);
        let thread_id = cx.create_rw_signal(None);
        let stack_traces: RwSignal<BTreeMap<ThreadId, StackTraceData>> =
            cx.create_rw_signal(BTreeMap::new());
        let breakline = cx.create_memo(move |_| {
            let thread_id = thread_id.get();
            if let Some(thread_id) = thread_id {
                let trace = stack_traces
                    .with(|stack_traces| stack_traces.get(&thread_id).cloned());

                if let Some(trace) = trace {
                    let breakline = trace.frames.with(|f| {
                        f.get(0)
                            .and_then(|f| {
                                f.source
                                    .as_ref()
                                    .map(|s| (f.line.saturating_sub(1), s))
                            })
                            .and_then(|(line, s)| s.path.clone().map(|p| (line, p)))
                    });
                    return breakline;
                }
                None
            } else {
                None
            }
        });
        Self {
            term_id,
            dap_id,
            stopped,
            thread_id,
            stack_traces,
            variables_id: cx.create_rw_signal(0),
            variables: cx.create_rw_signal(DapVariable {
                item:                    ScopeOrVar::Scope(
                    dap_types::Scope::default(),
                ),
                parent:                  Vec::new(),
                expanded:                true,
                read:                    true,
                children:                Vec::new(),
                children_expanded_count: 0,
            }),
            breakline,
            common,
        }
    }

    pub fn stopped(
        &self,
        cx: Scope,
        stopped: &Stopped,
        stack_traces: &HashMap<ThreadId, Vec<StackFrame>>,
        variables: &[(dap_types::Scope, Vec<Variable>)],
    ) {
        self.stopped.set(true);
        self.thread_id.update(|thread_id| {
            *thread_id = Some(stopped.thread_id.unwrap_or_default());
        });

        let main_thread_id = self.thread_id.get_untracked();
        let mut current_stack_traces = self.stack_traces.get_untracked();
        current_stack_traces.retain(|t, _| stack_traces.contains_key(t));
        for (thread_id, frames) in stack_traces {
            let is_main_thread = main_thread_id.as_ref() == Some(thread_id);
            if is_main_thread {
                if let Some(frame) = frames.first() {
                    if let Some(path) =
                        frame.source.as_ref().and_then(|source| source.path.clone())
                    {
                        self.common.internal_command.send(
                            InternalCommand::JumpToLocation {
                                location: EditorLocation {
                                    path,
                                    position: Some(EditorPosition::Line(
                                        frame.line.saturating_sub(1),
                                    )),
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                },
                            },
                        );
                    }
                }
            }
            if let Some(current) = current_stack_traces.get_mut(thread_id) {
                current.frames.set(frames.into());
                if is_main_thread {
                    current.expanded.set(true);
                }
            } else {
                current_stack_traces.insert(
                    *thread_id,
                    StackTraceData {
                        expanded:     cx.create_rw_signal(is_main_thread),
                        frames:       cx.create_rw_signal(frames.into()),
                        frames_shown: 20,
                    },
                );
            }
        }
        self.stack_traces.set(current_stack_traces);
        self.variables.update(|dap_var| {
            dap_var.children = variables
                .iter()
                .enumerate()
                .map(|(i, (scope, vars))| DapVariable {
                    item:                    ScopeOrVar::Scope(scope.to_owned()),
                    parent:                  Vec::new(),
                    expanded:                i == 0,
                    read:                    true,
                    children:                vars
                        .iter()
                        .map(|var| DapVariable {
                            item:                    ScopeOrVar::Var(var.to_owned()),
                            parent:                  vec![scope.variables_reference],
                            expanded:                false,
                            read:                    false,
                            children:                Vec::new(),
                            children_expanded_count: 0,
                        })
                        .collect(),
                    children_expanded_count: if i == 0 { vars.len() } else { 0 },
                })
                .collect();
            dap_var.children_expanded_count = dap_var
                .children
                .iter()
                .map(|v| v.children_expanded_count + 1)
                .sum::<usize>();
        });
    }

    pub fn toggle_expand(&self, parent: Vec<usize>, reference: usize) {
        self.variables_id.update(|id| {
            *id += 1;
        });
        self.variables.update(|variables| {
            if let Some(var) = variables.get_var_mut(&parent, reference) {
                if var.expanded {
                    var.expanded = false;
                    variables.update_count_recursive(&parent, reference);
                } else {
                    var.expanded = true;
                    if !var.read {
                        var.read = true;
                        self.read_var_children(&parent, reference);
                    } else {
                        variables.update_count_recursive(&parent, reference);
                    }
                }
            }
        });
    }

    fn read_var_children(&self, parent: &[usize], reference: usize) {
        let root = self.variables;
        let parent = parent.to_vec();
        let variables_id = self.variables_id;

        let send = create_ext_action(self.common.scope, move |result| {
            if let Ok(ProxyResponse::DapVariableResponse { varialbes }) = result {
                variables_id.update(|id| {
                    *id += 1;
                });
                root.update(|root| {
                    if let Some(var) = root.get_var_mut(&parent, reference) {
                        let mut new_parent = parent.clone();
                        new_parent.push(reference);
                        var.read = true;
                        var.children = varialbes
                            .into_iter()
                            .map(|v| DapVariable {
                                item:                    ScopeOrVar::Var(v),
                                parent:                  new_parent.clone(),
                                expanded:                false,
                                read:                    false,
                                children:                Vec::new(),
                                children_expanded_count: 0,
                            })
                            .collect();
                        root.update_count_recursive(&parent, reference);
                    }
                });
            }
        });
        self.common.proxy.proxy_rpc.dap_variable(
            self.dap_id,
            reference,
            move |(_, result)| {
                send(result);
            },
        );
    }
}

impl VirtualVector<DapVariableViewdata> for DapVariable {
    fn total_len(&self) -> usize {
        self.children_expanded_count
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<Item = DapVariableViewdata> {
        let min = range.start;
        let max = range.end;
        let mut i = 0;
        let mut view_items = Vec::new();
        for item in self.children.iter() {
            i = item.append_view_slice(&mut view_items, min, max, i + 1, 0);
            if i > max {
                return view_items.into_iter();
            }
        }

        view_items.into_iter()
    }
}

impl DapVariable {
    pub fn append_view_slice(
        &self,
        view_items: &mut Vec<DapVariableViewdata>,
        min: usize,
        max: usize,
        current: usize,
        level: usize,
    ) -> usize {
        if current > max {
            return current;
        }
        if current + self.children_expanded_count < min {
            return current + self.children_expanded_count;
        }

        let mut i = current;
        if current >= min {
            view_items.push(DapVariableViewdata {
                item: self.item.clone(),
                parent: self.parent.clone(),
                expanded: self.expanded,
                level,
            });
        }

        if self.expanded {
            for item in self.children.iter() {
                i = item.append_view_slice(view_items, min, max, i + 1, level + 1);
                if i > max {
                    return i;
                }
            }
        }
        i
    }

    pub fn get_var_mut(
        &mut self,
        parent: &[usize],
        reference: usize,
    ) -> Option<&mut DapVariable> {
        let parent = if parent.is_empty() {
            self
        } else {
            parent.iter().try_fold(self, |item, parent| {
                item.children
                    .iter_mut()
                    .find(|c| c.item.reference() == *parent)
            })?
        };
        parent
            .children
            .iter_mut()
            .find(|c| c.item.reference() == reference)
    }

    pub fn update_count_recursive(&mut self, parent: &[usize], reference: usize) {
        let mut parent = parent.to_vec();
        self.update_count(&parent, reference);
        while let Some(reference) = parent.pop() {
            self.update_count(&parent, reference);
        }
        self.children_expanded_count = self
            .children
            .iter()
            .map(|item| item.children_expanded_count + 1)
            .sum::<usize>();
    }

    pub fn update_count(
        &mut self,
        parent: &[usize],
        reference: usize,
    ) -> Option<()> {
        let var = self.get_var_mut(parent, reference)?;
        var.children_expanded_count = if var.expanded {
            var.children
                .iter()
                .map(|item| item.children_expanded_count + 1)
                .sum::<usize>()
        } else {
            0
        };
        Some(())
    }
}

pub fn update_breakpoints(
    daps: RwSignal<im::HashMap<DapId, DapData>>,
    proxy: ProxyRpcHandler,
    breakpoints: BreakPoints,
    action: BreakpointAction,
) {
    let (path_breakpoints, path) = match action {
        BreakpointAction::Remove { path, line } => {
            (breakpoints.remove_by_path_line(path, line), path)
        },
        BreakpointAction::Add { path, line, offset } => (
            breakpoints.add_by_path_line_offset(path, line, offset),
            path,
        ),
        BreakpointAction::Toggle { path, line } => {
            (breakpoints.toggle_by_path_line(path, line), path)
        },
        BreakpointAction::AddOrRemove { path, line, offset } => (
            breakpoints.add_or_remove_by_path_line_offset(path, line, offset),
            path,
        ),
    };

    let source_breakpoints: Vec<SourceBreakpoint> = path_breakpoints
        .iter()
        .filter_map(|(_, b)| {
            if b.active {
                Some(SourceBreakpoint {
                    line:          b.line + 1,
                    column:        None,
                    condition:     None,
                    hit_condition: None,
                    log_message:   None,
                })
            } else {
                None
            }
        })
        .collect();
    let daps: Vec<DapId> =
        daps.with_untracked(|daps| daps.keys().cloned().collect());
    for dap_id in daps {
        proxy.dap_set_breakpoints(
            dap_id,
            path.to_path_buf(),
            source_breakpoints.clone(),
        );
    }
}

#[cfg(test)]
mod tests {
    use lapce_rpc::dap_types::{Scope, Variable};

    use super::{DapVariable, ScopeOrVar};

    #[test]
    fn test_update_count() {
        let variables = vec![
            (
                Scope {
                    variables_reference: 0,
                    ..Default::default()
                },
                vec![
                    Variable {
                        variables_reference: 3,
                        ..Default::default()
                    },
                    Variable {
                        variables_reference: 4,
                        ..Default::default()
                    },
                ],
            ),
            (
                Scope {
                    variables_reference: 1,
                    ..Default::default()
                },
                vec![
                    Variable {
                        variables_reference: 5,
                        ..Default::default()
                    },
                    Variable {
                        variables_reference: 6,
                        ..Default::default()
                    },
                ],
            ),
            (
                Scope {
                    variables_reference: 2,
                    ..Default::default()
                },
                vec![
                    Variable {
                        variables_reference: 7,
                        ..Default::default()
                    },
                    Variable {
                        variables_reference: 8,
                        ..Default::default()
                    },
                ],
            ),
        ];

        let mut root = DapVariable {
            item:                    ScopeOrVar::Scope(Scope::default()),
            parent:                  Vec::new(),
            expanded:                true,
            read:                    true,
            children:                variables
                .iter()
                .map(|(scope, vars)| DapVariable {
                    item:                    ScopeOrVar::Scope(scope.to_owned()),
                    parent:                  Vec::new(),
                    expanded:                true,
                    read:                    true,
                    children:                vars
                        .iter()
                        .map(|var| DapVariable {
                            item:                    ScopeOrVar::Var(var.to_owned()),
                            parent:                  vec![scope.variables_reference],
                            expanded:                false,
                            read:                    false,
                            children:                Vec::new(),
                            children_expanded_count: 0,
                        })
                        .collect(),
                    children_expanded_count: vars.len(),
                })
                .collect(),
            children_expanded_count: 0,
        };
        root.children_expanded_count = root
            .children
            .iter()
            .map(|v| v.children_expanded_count + 1)
            .sum::<usize>();
        assert_eq!(root.children_expanded_count, 9);

        let var = root.get_var_mut(&[0], 3).unwrap();
        var.expanded = true;
        var.read = true;
        var.children = vec![
            Variable {
                variables_reference: 9,
                ..Default::default()
            },
            Variable {
                variables_reference: 10,
                ..Default::default()
            },
        ]
        .iter()
        .map(|var| DapVariable {
            item:                    ScopeOrVar::Var(var.to_owned()),
            parent:                  vec![0, 3],
            expanded:                false,
            read:                    false,
            children:                Vec::new(),
            children_expanded_count: 0,
        })
        .collect();
        root.update_count_recursive(&[0], 3);
        let var = root.get_var_mut(&[0], 3).unwrap();
        assert_eq!(var.children_expanded_count, 2);
        let var = root.get_var_mut(&[], 0).unwrap();
        assert_eq!(var.children_expanded_count, 4);
        assert_eq!(root.children_expanded_count, 11);
    }
}
