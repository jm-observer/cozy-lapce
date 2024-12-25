use std::{ops::AddAssign, rc::Rc};

use floem::{
    reactive::{RwSignal, SignalGet, SignalUpdate, SignalWith},
    style::CursorStyle,
    views::{
        container, empty, label, scroll, stack, virtual_stack, Decorators,
        VirtualDirection, VirtualItemSize, VirtualVector,
    },
    IntoView, View, ViewId,
};
use lsp_types::{CallHierarchyItem, Range};

use crate::common::common_tab_header;
use crate::panel::position::PanelContainerPosition;
use crate::{
    command::InternalCommand,
    config::{color::LapceColor, icon::LapceIcons},
    editor::location::EditorLocation,
    svg,
    window_tab::WindowTabData,
};

#[derive(Clone, Debug)]
pub struct CallHierarchyData {
    pub root: RwSignal<CallHierarchyItemData>,
    pub root_id: ViewId,
    // pub common: Rc<CommonData>,
    pub scroll_to_line: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CallHierarchyItemData {
    pub root_id: ViewId,
    pub view_id: ViewId,
    pub item: Rc<CallHierarchyItem>,
    pub from_range: Range,
    pub init: bool,
    pub open: RwSignal<bool>,
    pub children: RwSignal<Vec<RwSignal<CallHierarchyItemData>>>,
}

impl CallHierarchyItemData {
    pub fn child_count(&self) -> usize {
        let mut count = 1;
        if self.open.get() {
            for child in self.children.get_untracked() {
                count += child.with(|x| x.child_count())
            }
        }
        count
    }

    pub fn find_by_id(
        root: RwSignal<CallHierarchyItemData>,
        view_id: ViewId,
    ) -> Option<RwSignal<CallHierarchyItemData>> {
        if root.get_untracked().view_id == view_id {
            Some(root)
        } else {
            root.get_untracked()
                .children
                .get_untracked()
                .into_iter()
                .find_map(|x| Self::find_by_id(x, view_id))
        }
    }
}

fn get_children(
    data: RwSignal<CallHierarchyItemData>,
    next: &mut usize,
    min: usize,
    max: usize,
    level: usize,
) -> Vec<(usize, usize, RwSignal<CallHierarchyItemData>)> {
    let mut children = Vec::new();
    if *next >= min && *next < max {
        children.push((*next, level, data));
    } else if *next >= max {
        return children;
    }
    next.add_assign(1);
    if data.get_untracked().open.get() {
        for child in data.get().children.get_untracked() {
            let child_children = get_children(child, next, min, max, level + 1);
            children.extend(child_children);
            if *next > max {
                break;
            }
        }
    }
    children
}

pub struct VirtualList {
    root: Option<RwSignal<CallHierarchyItemData>>,
}

impl VirtualList {
    pub fn new(root: Option<RwSignal<CallHierarchyItemData>>) -> Self {
        Self { root }
    }
}

impl VirtualVector<(usize, usize, RwSignal<CallHierarchyItemData>)> for VirtualList {
    fn total_len(&self) -> usize {
        if let Some(root) = &self.root {
            root.with(|x| x.child_count())
        } else {
            0
        }
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<Item = (usize, usize, RwSignal<CallHierarchyItemData>)> {
        if let Some(root) = &self.root {
            let min = range.start;
            let max = range.end;
            let children = get_children(*root, &mut 0, min, max, 0);
            children.into_iter()
        } else {
            Vec::new().into_iter()
        }
    }
}

pub fn show_hierarchy_panel(
    window_tab_data: Rc<WindowTabData>,
    _position: PanelContainerPosition,
) -> impl View {
    stack((
        common_tab_header(
            window_tab_data.clone(),
            window_tab_data.main_split.hierarchy.clone(),
        ),
        _show_hierarchy_panel(window_tab_data.clone(), _position, move || {
            VirtualList::new(
                window_tab_data
                    .main_split
                    .hierarchy
                    .get_active_content()
                    .map(|x| x.root),
            )
        })
        .debug_name("show hierarchy panel"),
    ))
    .style(|x| x.flex_col().width_full().height_full())
}
pub fn _show_hierarchy_panel(
    window_tab_data: Rc<WindowTabData>,
    _position: PanelContainerPosition,
    each_fn: impl Fn() -> VirtualList + 'static,
) -> impl View {
    let config = window_tab_data.common.config;
    let ui_line_height = window_tab_data.common.ui_line_height;
    let scroll_to_line: RwSignal<Option<f64>> = RwSignal::new(None);
    scroll(
        virtual_stack(
            VirtualDirection::Vertical,
            VirtualItemSize::Fixed(Box::new(move || ui_line_height.get())),
            each_fn,
            move |(_, _, item)| item.get_untracked().view_id,
            move |(_, level, rw_data)| {
                let data = rw_data.get_untracked();
                let open = data.open;
                let kind = data.item.kind;
                stack((
                    container(
                        svg(move || {
                            let config = config.get();
                            let svg_str = match open.get() {
                                true => LapceIcons::ITEM_OPENED,
                                false => LapceIcons::ITEM_CLOSED,
                            };
                            config.ui_svg(svg_str)
                        })
                        .style(move |s| {
                            let config = config.get();
                            let size = config.ui.icon_size() as f32;
                            s.size(size, size)
                                .color(config.color(LapceColor::LAPCE_ICON_ACTIVE))
                        })
                    )
                    .style(|s| s.padding(4.0).margin_left(6.0).margin_right(2.0))
                    .on_click_stop({
                        let window_tab_data = window_tab_data.clone();
                        move |_x| {
                            open.update(|x| {
                                *x = !*x;
                            });
                            if !rw_data.get_untracked().init {
                                let data = rw_data.get_untracked();
                                window_tab_data.common.internal_command.send(
                                    InternalCommand::CallHierarchyIncoming {
                                        root_id: data.root_id,
                                        item_id: data.view_id,
                                    },
                                );
                            }
                        }
                    }),
                    svg(move || {
                        let config = config.get();
                        config
                            .symbol_svg(&kind)
                            .unwrap_or_else(|| config.ui_svg(LapceIcons::FILE))
                    }).style(move |s| {
                            let config = config.get();
                            let size = config.ui.icon_size() as f32;
                            s.min_width(size)
                                .size(size, size)
                                .margin_right(5.0)
                                .color(config.symbol_color(&kind).unwrap_or_else(|| {
                                    config.color(LapceColor::LAPCE_ICON_ACTIVE)
                                }))
                        }),
                    data.item.name.clone().into_view(),
                    if data.item.detail.is_some() {
                        label(move || {
                            data.item.detail.clone().unwrap_or_default().replace('\n', "↵")
                        }).style(move |s| s.margin_left(6.0)
                                                .color(config.get().color(LapceColor::EDITOR_DIM))
                        ).into_any()
                    } else {
                        empty().into_any()
                    },
                ))
                .style(move |s| {
                    s.padding_right(5.0)
                        .height(ui_line_height.get())
                        .padding_left((level * 10) as f32)
                        .items_center()
                        .hover(|s| {
                            s.background(
                                config
                                    .get()
                                    .color(LapceColor::PANEL_HOVERED_BACKGROUND),
                            )
                            .cursor(CursorStyle::Pointer)
                        })
                })
                .on_click_stop({
                    let window_tab_data = window_tab_data.clone();
                    let data = rw_data;
                    move |_| {
                        if !rw_data.get_untracked().init {
                            let data = rw_data.get_untracked();
                            window_tab_data.common.internal_command.send(
                                InternalCommand::CallHierarchyIncoming { item_id: data.view_id, root_id: data.root_id },
                            );
                        }
                        let data = data.get_untracked();
                        if let Ok(path) = data.item.uri.to_file_path() {
                            window_tab_data
                                .common
                                .internal_command
                                .send(InternalCommand::JumpToLocation { location: EditorLocation {
                                    path,
                                    position: Some(crate::editor::location::EditorPosition::Position(data.from_range.start)),
                                    scroll_offset: None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab: false,
                                } });
                        }
                    }
                })
            },
        )
        .style(|s| s.flex_col().absolute().min_width_full()),
    )
    .style(|s| s.size_full())
    .scroll_to(move || {
        if let Some(line) = scroll_to_line.get() {
            let line_height = ui_line_height.get();
            Some((0.0, line * line_height).into())
        } else {
            None
        }
    })
}
