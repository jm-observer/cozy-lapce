use std::{ops::AddAssign, path::PathBuf};

use floem::{
    IntoView, View, ViewId,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate},
    style::CursorStyle,
    views::{
        Decorators, VirtualVector, container, label, scroll, stack, svg,
        virtual_stack,
    },
};
use im::HashMap;
use itertools::Itertools;
use lapce_core::{icon::LapceIcons, panel::PanelContainerPosition};
use lapce_rpc::file_line::FileLine;
use lsp_types::{Location, SymbolKind, request::GotoImplementationResponse};

use crate::{
    command::InternalCommand,
    common::{TabHead, common_tab_header},
    config::color::LapceColor,
    editor::location::EditorLocation,
    window_workspace::WindowWorkspaceData,
};

pub fn implementation_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition,
) -> impl View {
    stack((
        common_tab_header(
            window_tab_data.clone(),
            window_tab_data.main_split.implementations,
        ),
        common_reference_panel(window_tab_data.clone(), _position, move || {
            window_tab_data
                .main_split
                .implementations
                .get_active_content()
                .unwrap_or_default()
        })
        .debug_name("implementation tabs"),
    ))
    .style(|x| x.flex_col().width_full())
    .debug_name("implementation panel")
}
pub fn common_reference_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition,
    each_fn: impl Fn() -> ReferencesRoot + 'static,
) -> impl View {
    let config = window_tab_data.common.config;
    let ui_line_height = window_tab_data.common.ui_line_height;
    scroll(
        virtual_stack(
            // VirtualDirection::Vertical,
            // VirtualItemSize::Fixed(Box::new(move || ui_line_height.get())),
            each_fn,
            move |(_, _, data)| data.view_id(),
            move |(_, level, rw_data)| {
                match rw_data {
                    ReferenceLocation::File { path, open, .. } => stack((
                        container(
                            svg(move || {
                                let svg_str = match open.get() {
                                    true => LapceIcons::ITEM_OPENED,
                                    false => LapceIcons::ITEM_CLOSED,
                                };
                                config.with_ui_svg(svg_str)
                            })
                            .style(move |s| {
                                let (caret_color, size) = config.signal(|config| {
                                    (
                                        config.color(LapceColor::LAPCE_ICON_ACTIVE), config.ui.icon_size.signal()
                                    )
                                });
                                let size = size.get() as f32;
                                s.size(size, size).color(caret_color.get())
                            }),
                        )
                        .style(|s| s.padding(4.0).margin_left(6.0).margin_right(2.0))
                        .on_click_stop({
                            move |_x| {
                                open.update(|x| {
                                    *x = !*x;
                                });
                            }
                        }),
                        svg(move || {
                            let (symbol_svg, file_svg) = config.signal(|config| {
                                (config.symbol_svg(SymbolKind::FILE), config.ui_svg(LapceIcons::FILE))
                            });
                            if let Some(svg) = symbol_svg {
                                svg.get()
                            } else {
                                file_svg.get()
                            }
                        })
                        .style(move |s| {
                            let (size, color) = config.signal(|config| {
                                (
                                    config.ui.icon_size.signal(), config
                                    .symbol_color(&SymbolKind::FILE)
                                    .unwrap_or(
                                        config
                                            .color(LapceColor::LAPCE_ICON_ACTIVE)
                                    )
                                )
                            });
                            let size = size.get() as f32;
                            s.min_width(size)
                                .size(size, size)
                                .margin_right(5.0)
                                .color(color.get()
                                )
                        }),
                        label(move || format!("{path:?}", ))
                            .style(move |s| {
                                s.margin_left(6.0).color(
                                    config.with_color(LapceColor::EDITOR_DIM),
                                )
                            })
                            .into_any(),
                    ))
                    .style(move |s| {
                        s.padding_right(5.0)
                            .height(ui_line_height.get())
                            .padding_left((level * 10) as f32)
                            .items_center()
                            .hover(|s| {
                                s.background(
                                    config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                                )
                                .cursor(CursorStyle::Pointer)
                            })
                    }),
                    ReferenceLocation::Line { file_line, .. } => stack((container(
                        label(move || format!("{} {}", file_line.position.line + 1, file_line.content))
                            .style(move |s| {
                                s.margin_left(6.0).color(
                                    config.with_color(LapceColor::EDITOR_DIM),
                                )
                            })
                            .into_any(),
                    )
                    .style(move |s| {
                        s.padding_right(5.0)
                            .height(ui_line_height.get())
                            .padding_left((level * 20) as f32)
                            .items_center()
                            .hover(|s| {
                                s.background(
                                    config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                                )
                                .cursor(CursorStyle::Pointer)
                            })
                    }),))
                    .on_click_stop({
                        let window_tab_data = window_tab_data.clone();
                        let position = file_line.position;
                        move |_| {
                            window_tab_data.common.internal_command.send(
                                InternalCommand::JumpToLocation {
                                    location: EditorLocation {
                                        path: file_line.path.clone(),
                                        position: Some(
                                            crate::editor::location::EditorPosition::Position(
                                                position,
                                            ),
                                        ),
                                        scroll_offset: None,
                                        ignore_unconfirmed: false,
                                        same_editor_tab: false,
                                    },
                                },
                            );
                        }
                    }),
                }
                .style(move |s| {
                    s.padding_right(5.0)
                        .height(ui_line_height.get())
                        .padding_left((level * 10) as f32)
                        .items_center()
                        .hover(|s| {
                            s.background(
                                config.with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                            )
                            .cursor(CursorStyle::Pointer)
                        })
                })
            },
        )
        .style(|s| s.flex_col().absolute().min_width_full()),
    )
        .style(|s| s.size_full())
}

pub fn map_to_location(resp: Option<GotoImplementationResponse>) -> Vec<Location> {
    let Some(resp) = resp else {
        return Vec::new();
    };
    match resp {
        GotoImplementationResponse::Scalar(local) => {
            vec![local]
        },
        GotoImplementationResponse::Array(items) => items,
        GotoImplementationResponse::Link(items) => items
            .into_iter()
            .map(|x| Location {
                uri:   x.target_uri,
                range: x.target_range,
            })
            .collect(),
    }
}

pub fn init_implementation_root(
    items: Vec<FileLine>,
    scope: Scope,
) -> ReferencesRoot {
    let mut refs_map: HashMap<PathBuf, HashMap<u32, Reference>> = HashMap::new();
    for item in items {
        let entry = refs_map.entry(item.path.clone()).or_default();
        (*entry).insert(
            item.position.line,
            Reference::Line {
                location: ReferenceLocation::Line {
                    file_line: item,
                    view_id:   ViewId::new(),
                },
            },
        );
    }

    let mut refs = Vec::new();
    for (path, items) in refs_map {
        let open = scope.create_rw_signal(true);
        let children = items
            .into_iter()
            .sorted_by(|x, y| x.0.cmp(&y.0))
            .map(|x| x.1)
            .collect();
        let ref_item = Reference::File {
            location: ReferenceLocation::File {
                open,
                path,
                view_id: ViewId::new(),
            },
            children,
            open,
        };
        refs.push(ref_item);
    }
    ReferencesRoot { children: refs }
}

#[derive(Clone, Default)]
pub struct ReferencesRoot {
    pub(crate) children: Vec<Reference>,
}

impl TabHead for ReferencesRoot {}

impl ReferencesRoot {
    pub fn total(&self) -> usize {
        let mut total = 0;
        for child in &self.children {
            total += child.total_len()
        }
        total
    }

    fn get_children(
        &self,
        next: &mut usize,
        min: usize,
        max: usize,
        level: usize,
    ) -> Vec<(usize, usize, ReferenceLocation)> {
        let mut children = Vec::new();
        for child in &self.children {
            let child_children = child.get_children(next, min, max, level + 1);
            if !child_children.is_empty() {
                children.extend(child_children);
            }
            if *next > max {
                break;
            }
        }
        children
    }
}

impl VirtualVector<(usize, usize, ReferenceLocation)> for ReferencesRoot {
    fn total_len(&self) -> usize {
        self.total()
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<Item = (usize, usize, ReferenceLocation)> {
        let min = range.start;
        let max = range.end;
        let children = self.get_children(&mut 0, min, max, 0);
        children.into_iter()
    }
}

#[derive(Clone)]
pub enum Reference {
    File {
        location: ReferenceLocation,
        open:     RwSignal<bool>,
        children: Vec<Reference>,
    },
    Line {
        location: ReferenceLocation,
    },
}

#[derive(Clone)]
pub enum ReferenceLocation {
    File {
        path:    PathBuf,
        open:    RwSignal<bool>,
        view_id: ViewId,
    },
    Line {
        file_line: FileLine,
        view_id:   ViewId,
    },
}

impl ReferenceLocation {
    pub fn view_id(&self) -> ViewId {
        match self {
            ReferenceLocation::File { view_id, .. } => *view_id,
            ReferenceLocation::Line { view_id, .. } => *view_id,
        }
    }
}

impl Reference {
    pub fn location(&self) -> ReferenceLocation {
        match self {
            Reference::File { location, .. } => location.clone(),
            Reference::Line { location } => location.clone(),
        }
    }

    pub fn total_len(&self) -> usize {
        match self {
            Reference::File { children, .. } => {
                let mut total = 1;
                for child in children {
                    total += child.total_len()
                }
                total
            },
            Reference::Line { .. } => 1,
        }
    }

    pub fn children(&self) -> Option<&Vec<Reference>> {
        match self {
            Reference::File { children, open, .. } => {
                if open.get() {
                    return Some(children);
                }
                None
            },
            Reference::Line { .. } => None,
        }
    }

    fn get_children(
        &self,
        next: &mut usize,
        min: usize,
        max: usize,
        level: usize,
    ) -> Vec<(usize, usize, ReferenceLocation)> {
        let mut children = Vec::new();
        if *next >= min && *next < max {
            children.push((*next, level, self.location()));
        } else if *next >= max {
            return children;
        }
        next.add_assign(1);
        if let Some(children_tmp) = self.children() {
            for child in children_tmp {
                let child_children = child.get_children(next, min, max, level + 1);
                if !child_children.is_empty() {
                    children.extend(child_children);
                }
                if *next > max {
                    break;
                }
            }
        }
        children
    }
}
