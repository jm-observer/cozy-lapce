use std::{
    collections::HashSet,
    rc::Rc,
    sync::{Arc, atomic::AtomicU64},
};

use anyhow::Result;
use doc::lines::{
    command::EditCommand, editor_command::CommandExecuted, mode::Mode,
};
use floem::{
    IntoView, View,
    action::show_context_menu,
    ext_event::create_ext_action,
    keyboard::Modifiers,
    kurbo::Rect,
    menu::{Menu, MenuItem},
    reactive::{
        RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
        create_memo, create_rw_signal, use_context,
    },
    style::CursorStyle,
    views::{
        Decorators, container, dyn_container, dyn_stack, empty, img, label,
        rich_text, scroll, stack, svg, text,
    },
};
use indexmap::IndexMap;
use lapce_proxy::plugin::volt_icon;
use lapce_rpc::{
    core::{CoreNotification, CoreRpcHandler},
    plugin::{VoltID, VoltInfo, VoltMetadata},
};
use log::error;
use lsp_types::MessageType;
use serde::{Deserialize, Serialize};

use crate::{
    command::CommandKind,
    config::color::LapceColor,
    db::LapceDb,
    keypress::{KeyPressFocus, condition::Condition},
    local_task::{LocalRequest, LocalResponse},
    markdown::{MarkdownContent, parse_markdown},
    panel::plugin_view::VOLT_DEFAULT_PNG,
    web_link::web_link,
    window_workspace::CommonData,
};

type PluginInfo = Option<(
    Option<VoltMetadata>,
    VoltInfo,
    Option<VoltIcon>,
    Option<VoltInfo>,
    Option<RwSignal<bool>>,
)>;

#[derive(Clone, PartialEq, Eq)]
pub enum VoltIcon {
    Svg(String),
    Img(Vec<u8>),
}

impl VoltIcon {
    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        if let Ok(s) = std::str::from_utf8(buf) {
            Ok(VoltIcon::Svg(s.to_string()))
        } else {
            Ok(VoltIcon::Img(buf.to_vec()))
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct VoltsInfo {
    pub plugins: Vec<VoltInfo>,
    pub total:   usize,
}

#[derive(Clone)]
pub struct InstalledVoltData {
    pub meta:   RwSignal<VoltMetadata>,
    pub icon:   RwSignal<Option<VoltIcon>>,
    pub latest: RwSignal<VoltInfo>,
}

#[derive(Clone, PartialEq)]
pub struct AvailableVoltData {
    pub info:       RwSignal<VoltInfo>,
    pub icon:       RwSignal<Option<VoltIcon>>,
    pub installing: RwSignal<bool>,
}

#[derive(Clone, Debug)]
pub struct AvailableVoltList {
    pub loading:   RwSignal<bool>,
    pub query_id:  RwSignal<usize>,
    pub query_str: RwSignal<String>,
    pub volts:     RwSignal<IndexMap<VoltID, AvailableVoltData>>,
    pub total:     RwSignal<usize>,
}

#[derive(Clone, Debug)]
pub struct PluginData {
    pub installed:          RwSignal<IndexMap<VoltID, InstalledVoltData>>,
    pub available:          AvailableVoltList,
    pub all:                RwSignal<im::HashMap<VoltID, AvailableVoltData>>,
    pub disabled:           RwSignal<HashSet<VoltID>>,
    pub workspace_disabled: RwSignal<HashSet<VoltID>>,
    pub common:             Rc<CommonData>,
}

impl KeyPressFocus for PluginData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(condition, Condition::PanelFocus)
    }

    fn run_command(
        &self,
        command: &crate::command::LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_) => {},
            CommandKind::Scroll(_) => {},
            CommandKind::Focus(_) => {},
            CommandKind::Edit(_)
            | CommandKind::Move(_)
            | CommandKind::MultiSelection(_) => {
                #[allow(clippy::single_match)]
                match command.kind {
                    CommandKind::Edit(EditCommand::InsertNewLine) => {
                        return CommandExecuted::Yes;
                    },
                    _ => {},
                };
            },
            CommandKind::MotionMode(_) | CommandKind::Other(_) => (),
        }
        CommandExecuted::No
    }

    fn receive_char(&self, _c: &str) {}
}

impl PluginData {
    pub fn new(
        cx: Scope,
        disabled: HashSet<VoltID>,
        workspace_disabled: HashSet<VoltID>,
        common: Rc<CommonData>,
        core_rpc: CoreRpcHandler,
    ) -> Self {
        let installed = cx.create_rw_signal(IndexMap::new());
        let available = AvailableVoltList {
            loading:   cx.create_rw_signal(false),
            volts:     cx.create_rw_signal(IndexMap::new()),
            total:     cx.create_rw_signal(0),
            query_id:  cx.create_rw_signal(0),
            query_str: cx.create_rw_signal(String::new()),
        };
        let disabled = cx.create_rw_signal(disabled);
        let workspace_disabled = cx.create_rw_signal(workspace_disabled);

        let plugin = Self {
            installed,
            available,
            all: cx.create_rw_signal(im::HashMap::new()),
            disabled,
            workspace_disabled,
            common,
        };

        plugin.load_available_volts("", 0, core_rpc.clone());

        {
            let plugin = plugin.clone();
            let extra_plugin_paths =
                plugin.common.window_common.extra_plugin_paths.clone();
            let common = plugin.common.clone();
            let send =
                create_ext_action(
                    cx,
                    move |volts: Result<LocalResponse>| match volts {
                        Ok(response) => {
                            if let LocalResponse::FindAllVolts { volts } = response {
                                for meta in volts {
                                    if meta.wasm.is_none() {
                                        let icon = volt_icon(&meta);
                                        plugin.volt_installed(&meta, &icon);
                                    } else {
                                        continue;
                                    }
                                }
                            }
                        },
                        Err(err) => {
                            error!("{err:?}")
                        },
                    },
                );
            common.local_task.request_async(
                LocalRequest::FindAllVolts { extra_plugin_paths },
                move |(_id, rs)| {
                    send(rs);
                },
            );
        }

        {
            let plugin = plugin.clone();
            cx.create_effect(move |s| {
                let query = plugin.available.query_str.get();
                if s.as_ref() == Some(&query) {
                    return query;
                }
                // 优化限制的条件，延迟请求？
                plugin.available.query_id.update(|id| *id += 1);
                plugin.available.loading.set(false);
                plugin.available.volts.update(|v| v.clear());
                plugin.load_available_volts(&query, 0, core_rpc.clone());
                query
            });
        }

        plugin
    }

    pub fn volt_installed(&self, volt: &VoltMetadata, icon: &Option<Vec<u8>>) {
        let volt_id = volt.id();
        let (existing, is_latest, volt_data) = self
            .installed
            .try_update(|installed| {
                if let Some(v) = installed.get(&volt_id) {
                    (true, true, v.to_owned())
                } else {
                    let (info, is_latest) = if let Some(volt) = self
                        .available
                        .volts
                        .with_untracked(|all| all.get(&volt_id).cloned())
                    {
                        (volt.info.get_untracked(), true)
                    } else {
                        (volt.info(), false)
                    };

                    let latest = self.common.scope.create_rw_signal(info);
                    let data = InstalledVoltData {
                        meta: self.common.scope.create_rw_signal(volt.clone()),
                        icon: self.common.scope.create_rw_signal(
                            icon.as_ref()
                                .and_then(|icon| VoltIcon::from_bytes(icon).ok()),
                        ),
                        latest,
                    };
                    installed.insert(volt_id, data.clone());

                    (false, is_latest, data)
                }
            })
            .unwrap();

        if existing {
            volt_data.meta.set(volt.clone());
            volt_data.icon.set(
                icon.as_ref()
                    .and_then(|icon| VoltIcon::from_bytes(icon).ok()),
            );
        }

        let latest = volt_data.latest;
        if !is_latest {
            let send = create_ext_action(self.common.scope, move |info| {
                latest.set(info);
            });
            self.common.local_task.request_async(
                LocalRequest::QueryVoltInfo { meta: volt.clone() },
                move |(_id, rs)| match rs {
                    Ok(response) => {
                        if let LocalResponse::QueryVoltInfo { info } = response {
                            send(info);
                        }
                    },
                    Err(err) => {
                        error!("{}", err.to_string())
                    },
                },
            );
        }
    }

    pub fn volt_removed(&self, volt: &VoltInfo) {
        let id = volt.id();
        self.installed.update(|installed| {
            installed.swap_remove(&id);
        });

        if self.disabled.with_untracked(|d| d.contains(&id)) {
            self.disabled.update(|d| {
                d.remove(&id);
            });
            let db: Arc<LapceDb> = use_context().unwrap();
            db.save_disabled_volts(
                self.disabled.get_untracked().into_iter().collect(),
                &self.common.local_task,
            );
        }

        if self.workspace_disabled.with_untracked(|d| d.contains(&id)) {
            self.workspace_disabled.update(|d| {
                d.remove(&id);
            });
            let db: Arc<LapceDb> = use_context().unwrap();
            db.save_workspace_disabled_volts(
                self.common.workspace.clone(),
                self.workspace_disabled
                    .get_untracked()
                    .into_iter()
                    .collect(),
                &self.common.local_task,
            );
        }
    }

    fn load_available_volts(
        &self,
        query: &str,
        offset: usize,
        core_rpc: CoreRpcHandler,
    ) {
        if self.available.loading.get_untracked() {
            return;
        }
        self.available.loading.set(true);

        let volts = self.available.volts;
        let volts_total = self.available.total;
        let cx = self.common.scope;
        let loading = self.available.loading;
        let query_id = self.available.query_id;
        let current_query_id = self.available.query_id.get_untracked();
        let all = self.all;
        // let local_task = self.common.local_task.clone();
        // let core_rpc_clone = core_rpc.clone();
        let send = create_ext_action(self.common.scope, move |new: VoltsInfo| {
            loading.set(false);
            if query_id.get_untracked() != current_query_id {
                return;
            }
            let plugins = new.plugins.into_iter().map(|volt| {
                let icon_signal = cx.create_rw_signal(None);
                // let send = create_ext_action(cx, move |icon| {
                //     icon_signal.set(Some(icon));
                // });
                // {
                //     let info = volt.clone();
                //     let core_rpc = core_rpc_clone.clone();
                //     local_task.request_async(
                //         LocalRequest::LoadIcon { info },
                //         move |(_id, rs)| match rs {
                //             Ok(response) => {
                //                 if let LocalResponse::LoadIcon { icon } = response
                // {                     send(icon);
                //                 }
                //             },
                //             Err(err) => {
                //                 core_rpc.notification(
                //                     CoreNotification::ShowMessage {
                //                         title:   "Load Plugin Icon".to_string(),
                //                         message: lsp_types::ShowMessageParams {
                //                             typ:     MessageType::ERROR,
                //                             message: err.to_string()
                //                         }
                //                     }
                //                 );
                //             }
                //         }
                //     );
                // }

                let data = AvailableVoltData {
                    info:       cx.create_rw_signal(volt.clone()),
                    icon:       icon_signal,
                    installing: cx.create_rw_signal(false),
                };
                all.update(|all| {
                    all.insert(volt.id(), data.clone());
                });

                (volt.id(), data)
            });
            volts.update(|volts| {
                volts.extend(plugins);
            });
            volts_total.set(new.total);
        });

        let query = query.to_string();
        let core_rpc = core_rpc.clone();
        self.common.local_task.request_async(
            LocalRequest::QueryVolts { query, offset },
            move |(_id, rs)| match rs {
                Ok(response) => {
                    if let LocalResponse::QueryVolts { volts } = response {
                        send(volts);
                    }
                },
                Err(err) => {
                    core_rpc.notification(CoreNotification::ShowMessage {
                        title:   "Request Available Plugins".to_string(),
                        message: lsp_types::ShowMessageParams {
                            typ:     MessageType::ERROR,
                            message: err.to_string(),
                        },
                    });
                    error!("{err}")
                },
            },
        );
    }

    fn all_loaded(&self) -> bool {
        self.available.volts.with_untracked(|v| v.len())
            >= self.available.total.get_untracked()
    }

    pub fn load_more_available(&self, core_rpc: CoreRpcHandler) {
        if self.all_loaded() {
            return;
        }

        let query = self.available.query_str.get_untracked();
        let offset = self.available.volts.with_untracked(|v| v.len());
        self.load_available_volts(&query, offset, core_rpc);
    }

    pub fn install_volt(&self, info: VoltInfo) {
        self.available.volts.with_untracked(|volts| {
            if let Some(volt) = volts.get(&info.id()) {
                volt.installing.set(true);
            };
        });
        if info.wasm {
            self.common.proxy.proxy_rpc.install_volt(info);
        } else {
            let plugin = self.clone();
            let send = create_ext_action(self.common.scope, move |(meta, icon)| {
                plugin.volt_installed(&meta, &icon);
            });
            self.common.local_task.request_async(
                LocalRequest::InstallVolt { info },
                move |(_id, rs)| match rs {
                    Ok(response) => {
                        if let LocalResponse::InstallVolt { volt, icon } = response {
                            send((volt, icon));
                        }
                    },
                    Err(err) => {
                        error!("{err:?}")
                    },
                },
            );
        }
    }

    pub fn plugin_disabled(&self, id: &VoltID) -> bool {
        self.disabled.with_untracked(|d| d.contains(id))
            || self.workspace_disabled.with_untracked(|d| d.contains(id))
    }

    pub fn enable_volt(&self, volt: VoltInfo) {
        let id = volt.id();
        self.disabled.update(|d| {
            d.remove(&id);
        });
        if !self.plugin_disabled(&id) {
            self.common.proxy.proxy_rpc.enable_volt(volt);
        }
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_disabled_volts(
            self.disabled.get_untracked().into_iter().collect(),
            &self.common.local_task,
        );
    }

    pub fn disable_volt(&self, volt: VoltInfo) {
        let id = volt.id();
        self.disabled.update(|d| {
            d.insert(id);
        });
        self.common.proxy.proxy_rpc.disable_volt(volt);
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_disabled_volts(
            self.disabled.get_untracked().into_iter().collect(),
            &self.common.local_task,
        );
    }

    pub fn enable_volt_for_ws(&self, volt: VoltInfo) {
        let id = volt.id();
        self.workspace_disabled.update(|d| {
            d.remove(&id);
        });
        if !self.plugin_disabled(&id) {
            self.common.proxy.proxy_rpc.enable_volt(volt);
        }
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_workspace_disabled_volts(
            self.common.workspace.clone(),
            self.disabled.get_untracked().into_iter().collect(),
            &self.common.local_task,
        );
    }

    pub fn disable_volt_for_ws(&self, volt: VoltInfo) {
        let id = volt.id();
        self.workspace_disabled.update(|d| {
            d.insert(id);
        });
        self.common.proxy.proxy_rpc.disable_volt(volt);
        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_workspace_disabled_volts(
            self.common.workspace.clone(),
            self.disabled.get_untracked().into_iter().collect(),
            &self.common.local_task,
        );
    }

    pub fn uninstall_volt(&self, volt: VoltMetadata) {
        if volt.wasm.is_some() {
            self.common.proxy.proxy_rpc.remove_volt(volt);
        } else if let Some(dir) = &volt.dir {
            let plugin = self.clone();
            let info = volt.info();
            let send = create_ext_action(self.common.scope, move |_| {
                plugin.volt_removed(&info);
            });
            let dir = dir.clone();
            self.common.local_task.request_async(
                LocalRequest::UninstallVolt { dir },
                move |(_id, rs)| match rs {
                    Ok(response) => {
                        if let LocalResponse::UninstallVolt = response {
                            send(());
                        }
                    },
                    Err(err) => {
                        error!("{err:?}")
                    },
                },
            );
        }
    }

    pub fn reload_volt(&self, volt: VoltMetadata) {
        self.common.proxy.proxy_rpc.reload_volt(volt);
    }

    pub fn plugin_controls(&self, meta: VoltMetadata, latest: VoltInfo) -> Menu {
        let volt_id = meta.id();
        let mut menu = Menu::new("");
        if meta.version != latest.version {
            menu = menu
                .entry(MenuItem::new("Upgrade Plugin").action({
                    let plugin = self.clone();
                    let info = latest.clone();
                    move || {
                        plugin.install_volt(info.clone());
                    }
                }))
                .separator();
        }
        menu = menu
            .entry(MenuItem::new("Reload Plugin").action({
                let plugin = self.clone();
                let meta = meta.clone();
                move || {
                    plugin.reload_volt(meta.clone());
                }
            }))
            .separator()
            .entry(
                MenuItem::new("Enable")
                    .enabled(
                        self.disabled
                            .with_untracked(|disabled| disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.enable_volt(volt.clone());
                        }
                    }),
            )
            .entry(
                MenuItem::new("Disable")
                    .enabled(
                        self.disabled
                            .with_untracked(|disabled| !disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.disable_volt(volt.clone());
                        }
                    }),
            )
            .separator()
            .entry(
                MenuItem::new("Enable For Workspace")
                    .enabled(
                        self.workspace_disabled
                            .with_untracked(|disabled| disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.enable_volt_for_ws(volt.clone());
                        }
                    }),
            )
            .entry(
                MenuItem::new("Disable For Workspace")
                    .enabled(
                        self.workspace_disabled
                            .with_untracked(|disabled| !disabled.contains(&volt_id)),
                    )
                    .action({
                        let plugin = self.clone();
                        let volt = meta.info();
                        move || {
                            plugin.disable_volt_for_ws(volt.clone());
                        }
                    }),
            )
            .separator()
            .entry(MenuItem::new("Uninstall").action({
                let plugin = self.clone();
                move || {
                    plugin.uninstall_volt(meta.clone());
                }
            }));
        menu
    }
}

pub fn plugin_info_view(plugin: PluginData, volt: VoltID) -> impl View {
    let config = plugin.common.config;
    let header_rect = create_rw_signal(Rect::ZERO);
    let scroll_width: RwSignal<f64> = create_rw_signal(0.0);
    let internal_command = plugin.common.internal_command;
    let local_plugin = plugin.clone();
    let directory = plugin.common.directory.clone();
    let local_task = plugin.common.local_task.clone();
    let plugin_info = create_memo(move |_| {
        plugin
            .installed
            .with(|volts| {
                volts.get(&volt).map(|v| {
                    (
                        Some(v.meta.get()),
                        v.meta.get().info(),
                        v.icon.get(),
                        Some(v.latest.get()),
                        None,
                    )
                })
            })
            .or_else(|| {
                plugin.all.with(|volts| {
                    volts.get(&volt).map(|v| {
                        (None, v.info.get(), v.icon.get(), None, Some(v.installing))
                    })
                })
            })
    });

    let version_view = move |plugin: PluginData, plugin_info: PluginInfo| {
        let version_info = plugin_info.as_ref().map(|(_, volt, _, latest, _)| {
            (
                volt.version.clone(),
                latest.as_ref().map(|i| i.version.clone()),
            )
        });
        let installing = plugin_info
            .as_ref()
            .and_then(|(_, _, _, _, installing)| *installing);
        let local_version_info = version_info.clone();
        let control = {
            move |version_info: Option<(String, Option<String>)>| match version_info
                .as_ref()
                .map(|(v, l)| match l {
                    Some(l) => (true, l == v),
                    None => (false, false),
                }) {
                Some((true, true)) => "Installed ▼",
                Some((true, false)) => "Upgrade ▼",
                _ => {
                    if installing.map(|i| i.get()).unwrap_or(false) {
                        "Installing"
                    } else {
                        "Install"
                    }
                },
            }
        };
        let local_plugin_info = plugin_info.clone();
        let local_plugin = plugin.clone();
        stack((
            text(
                version_info
                    .as_ref()
                    .map(|(v, _)| format!("v{v}"))
                    .unwrap_or_default(),
            ),
            label(move || control(local_version_info.clone()))
                .style(move |s| {
                    let (fg, bg, dim) = config.signal(|config| {
                        (
                            config
                                .color(LapceColor::LAPCE_BUTTON_PRIMARY_FOREGROUND),
                            config
                                .color(LapceColor::LAPCE_BUTTON_PRIMARY_BACKGROUND),
                            config.color(LapceColor::EDITOR_DIM),
                        )
                    });
                    let bg = bg.get();
                    s.margin_left(10)
                        .padding_horiz(10)
                        .border_radius(6.0)
                        .color(fg.get())
                        .background(bg)
                        .hover(|s| {
                            s.cursor(CursorStyle::Pointer)
                                .background(bg.multiply_alpha(0.8))
                        })
                        .active(|s| s.background(bg.multiply_alpha(0.6)))
                        .disabled(|s| s.background(dim.get()))
                        .selectable(false)
                })
                .disabled(move || installing.map(|i| i.get()).unwrap_or(false))
                .on_click_stop(move |_| {
                    if let Some((meta, info, _, latest, _)) =
                        local_plugin_info.as_ref()
                    {
                        if let Some(meta) = meta {
                            let menu = local_plugin.plugin_controls(
                                meta.to_owned(),
                                latest.clone().unwrap_or_else(|| info.to_owned()),
                            );
                            show_context_menu(menu, None);
                        } else {
                            local_plugin.install_volt(info.to_owned());
                        }
                    }
                }),
        ))
    };

    scroll(
        dyn_container(
            move || plugin_info.get(),
            move |plugin_info| {
                stack((
                    stack((
                        match plugin_info
                            .as_ref()
                            .and_then(|(_, _, icon, _, _)| icon.clone())
                        {
                            None => container(
                                img(move || VOLT_DEFAULT_PNG.to_vec())
                                    .style(|s| s.size_full()),
                            ),
                            Some(VoltIcon::Svg(svg_str)) => container(
                                svg(move || svg_str.clone())
                                    .style(|s| s.size_full()),
                            ),
                            Some(VoltIcon::Img(buf)) => container(
                                img(move || buf.clone()).style(|s| s.size_full()),
                            ),
                        }
                        .style(|s| {
                            s.min_size(150.0, 150.0).size(150.0, 150.0).padding(20)
                        }),
                        stack((
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| {
                                        volt.display_name.as_str()
                                    })
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                s.font_bold().font_size(
                                    (config.with_font_size() as f32 * 1.6)
                                        .round(),
                                )
                            }),
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| {
                                        volt.description.as_str()
                                    })
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                let scroll_width = scroll_width.get();
                                s.max_width(
                                    scroll_width
                                        .clamp(200.0 + 60.0 * 2.0 + 200.0, 800.0)
                                        - 60.0 * 2.0
                                        - 200.0,
                                )
                            }),
                            {
                                let repo = plugin_info
                                    .as_ref()
                                    .and_then(|(_, volt, _, _, _)| {
                                        volt.repository.as_deref()
                                    })
                                    .unwrap_or("")
                                    .to_string();
                                let local_repo = repo.clone();
                                stack((
                                    text("Repository: "),
                                    web_link(
                                        move || repo.clone(),
                                        move || local_repo.clone(),
                                        move || {
                                            config
                                                .with_color(LapceColor::EDITOR_LINK)
                                        },
                                        internal_command,
                                    ),
                                ))
                            },
                            text(
                                plugin_info
                                    .as_ref()
                                    .map(|(_, volt, _, _, _)| volt.author.as_str())
                                    .unwrap_or(""),
                            )
                            .style(move |s| {
                                s.color(config.with_color(LapceColor::EDITOR_DIM))
                            }),
                            version_view(local_plugin.clone(), plugin_info.clone()),
                        ))
                        .style(|s| s.flex_col().line_height(1.6)),
                    ))
                    .style(|s| s.absolute())
                    .on_resize(move |rect| {
                        if header_rect.get_untracked() != rect {
                            header_rect.set(rect);
                        }
                    }),
                    empty().style(move |s| {
                        let rect = header_rect.get();
                        s.size(rect.width(), rect.height())
                    }),
                    empty().style(move |s| {
                        s.margin_vert(6)
                            .height(1)
                            .width_full()
                            .background(config.with_color(LapceColor::LAPCE_BORDER))
                    }),
                    {
                        let readme = create_rw_signal(None);
                        let info = plugin_info
                            .as_ref()
                            .map(|(_, info, _, _, _)| info.to_owned());
                        let local_task = local_task.clone();
                        create_effect(move |_| {
                            let info = info.clone();
                            if let Some(info) = info {
                                let cx = Scope::current();
                                let send = create_ext_action(cx, move |md| {
                                        readme.set(Some(md));
                                });
                                local_task.request_async(
                                    LocalRequest::DownloadVoltReadme { info },
                                    move |(_id, rs)| match rs {
                                        Ok(response) => {
                                            if let LocalResponse::DownloadVoltReadme { readme } = response {
                                                send(readme);
                                            }
                                        },
                                        Err(err) => {
                                            error!("{err:?}")
                                        },
                                    },
                                );
                            }
                        });
                        {
                            let id = AtomicU64::new(0);
                            let directory = directory.clone();
                            dyn_stack(
                                move || {
                                    // todo improve "Loading README"
                                    let (font_family, editor_fg, font_size, markdown_blockquote, editor_link) = config.signal(|config| {
                                        (
                                            config.editor.font_family.signal(),
                                            config.color(LapceColor::EDITOR_FOREGROUND),
                                            config.ui.font_size.signal(),
                                            config.color(LapceColor::MARKDOWN_BLOCKQUOTE), config.color(LapceColor::EDITOR_LINK)
                                        )
                                    });
                                    let style_colors = config.with(|x| x.style_colors());
                                    let font_family = font_family.get();
                                    readme.get().unwrap_or_else(|| {
                                        parse_markdown(
                                            "Loading README",
                                            2.0,
                                            &directory, &font_family.0, editor_fg.get(), &style_colors, font_size.get() as f32, markdown_blockquote.get(), editor_link.get()
                                        )
                                    })
                                },
                                move |_| {
                                    id.fetch_add(
                                        1,
                                        std::sync::atomic::Ordering::Relaxed,
                                    )
                                },
                                move |content| match content {
                                    MarkdownContent::Text(text_layout) => container(
                                        rich_text(move || text_layout.clone())
                                            .style(|s| s.width_full()),
                                    )
                                    .style(|s| s.width_full()),
                                    MarkdownContent::Image { .. } => {
                                        container(empty())
                                    },
                                    MarkdownContent::Separator => {
                                        container(empty().style(move |s| {
                                            s.width_full()
                                                .margin_vert(5.0)
                                                .height(1.0)
                                                .background(
                                                    config.with_color(
                                                        LapceColor::LAPCE_BORDER,
                                                    ),
                                                )
                                        }))
                                    },
                                },
                            )
                            .style(|s| s.flex_col().width_full())
                        }
                    },
                ))
                .style(move |s| {
                    let padding = 60.0;
                    s.flex_col()
                        .width(
                            scroll_width
                                .get()
                                .min(800.0)
                                .max(header_rect.get().width() + padding * 2.0),
                        )
                        .padding(padding)
                })
                .into_any()
            },
        )
        .style(|s| s.min_width_full().justify_center()),
    )
    .on_resize(move |rect| {
        if scroll_width.get_untracked() != rect.width() {
            scroll_width.set(rect.width());
        }
    })
    .style(|s| s.absolute().size_full())
    .debug_name("Plugin Info")
}
