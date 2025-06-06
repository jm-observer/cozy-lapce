use std::{ops::AddAssign, path::PathBuf, rc::Rc};

use floem::{
    View,
    kurbo::Rect,
    peniko::Color,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
    style::CursorStyle,
    views::{
        Decorators, VirtualVector, container, h_stack, label, scroll, stack, svg,
        virtual_stack,
    },
};
use lapce_core::{icon::LapceIcons, id::Id, panel::PanelContainerPosition};
use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

use crate::{
    command::InternalCommand, common_svg, config::color::LapceColor,
    editor::location::EditorLocation, window_workspace::WindowWorkspaceData,
};

#[derive(Clone, Debug)]
pub struct SymbolData {
    pub path: PathBuf,
    pub file: RwSignal<SymbolInformationItemData>,
}

impl SymbolData {
    pub fn new(
        items: Vec<RwSignal<SymbolInformationItemData>>,
        path: PathBuf,
        cx: Scope,
    ) -> Self {
        let name = path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .unwrap_or_default();

        let end = items.iter().fold(Position::new(0, 0), |x, y| {
            let item_end = y.with_untracked(|x| x.item.range.end);
            if item_end > x { item_end } else { x }
        });
        #[allow(deprecated)]
        let file_ds = DocumentSymbol {
            name:            name.clone(),
            detail:          None,
            kind:            SymbolKind::FILE,
            tags:            None,
            deprecated:      None,
            range:           Range::new(Position::new(0, 0), end),
            selection_range: Default::default(),
            children:        None,
        };
        let file = cx.create_rw_signal(SymbolInformationItemData {
            id: Id::next(),
            name,
            detail: None,
            item: file_ds,
            open: cx.create_rw_signal(true),
            children: items,
        });
        Self { path, file }
    }

    fn get_children(
        &self,
        min: usize,
        max: usize,
    ) -> Vec<(
        usize,
        usize,
        Rc<PathBuf>,
        RwSignal<SymbolInformationItemData>,
    )> {
        let path = Rc::new(self.path.clone());
        let level: usize = 0;
        let mut next = 0;
        get_children(self.file, &mut next, min, max, level, path.clone())
    }

    /// line_index: start from 0
    /// MatchDocumentSymbol: start from 1
    pub fn match_line_with_children(&self, line: u32) -> MatchDocumentSymbol {
        self.file
            .with_untracked(|x| x.match_line_index_with_children(line))
    }
}

#[derive(Debug, Clone)]
pub struct SymbolInformationItemData {
    pub id:       Id,
    pub name:     String,
    pub detail:   Option<String>,
    pub item:     DocumentSymbol,
    pub open:     RwSignal<bool>,
    pub children: Vec<RwSignal<SymbolInformationItemData>>,
}

impl SymbolInformationItemData {
    pub fn new(mut item: DocumentSymbol, cx: Scope) -> Option<Self> {
        if matches!(item.kind, SymbolKind::VARIABLE) {
            None
        } else {
            let children = if let Some(children) = item.children.take() {
                children
                    .into_iter()
                    .filter_map(|x| Self::new(x, cx).map(|x| cx.create_rw_signal(x)))
                    .collect()
            } else {
                Vec::with_capacity(0)
            };
            Some(Self {
                id: Id::next(),
                name: item.name.clone(),
                detail: item.detail.clone(),
                item,
                open: cx.create_rw_signal(false),
                children,
            })
        }
    }
}

impl SymbolInformationItemData {
    pub fn child_count(&self) -> usize {
        let mut count = 1;
        if self.open.get() {
            for child in &self.children {
                count += child.with(|x| x.child_count())
            }
        }
        count
    }

    pub fn child_count_untracked(&self) -> usize {
        let mut count = 1;
        if self.open.get_untracked() {
            for child in &self.children {
                count += child.with_untracked(|x| x.child_count())
            }
        }
        count
    }

    pub fn find_by_name(&self, name: &str) -> Option<SymbolInformationItemData> {
        if self.name == name {
            return Some(self.clone());
        } else {
            for child in &self.children {
                let rs = child.with_untracked(|x| x.find_by_name(name));
                if rs.is_some() {
                    return rs;
                }
            }
        }
        None
    }

    /// line_index: start from 0
    /// MatchDocumentSymbol: start from 1
    pub fn match_line_index_with_children(
        &self,
        line_index: u32,
    ) -> MatchDocumentSymbol {
        let rs = self.match_line(line_index);
        match rs {
            MatchDocumentSymbol::LineBeforeSymbol => {
                MatchDocumentSymbol::LineBeforeSymbol
            },
            MatchDocumentSymbol::MatchSymbol(id, _) => {
                let mut all_line = 1;
                for child in self.children.iter() {
                    let rs_child = child.with_untracked(|x| {
                        x.match_line_index_with_children(line_index)
                    });
                    match rs_child {
                        MatchDocumentSymbol::LineBeforeSymbol => break,
                        MatchDocumentSymbol::MatchSymbol(id, count) => {
                            self.open.set(true);
                            return MatchDocumentSymbol::MatchSymbol(
                                id,
                                count + all_line,
                            );
                        },
                        MatchDocumentSymbol::LineAfterSymbol(count) => {
                            all_line += count;
                        },
                    }
                }
                MatchDocumentSymbol::MatchSymbol(id, all_line)
            },
            MatchDocumentSymbol::LineAfterSymbol(_) => {
                MatchDocumentSymbol::LineAfterSymbol(self.child_count_untracked())
            },
        }
    }

    fn match_line(&self, line: u32) -> MatchDocumentSymbol {
        if self.item.range.start.line > line {
            MatchDocumentSymbol::LineBeforeSymbol
        } else if self.item.range.start.line <= line
            && self.item.range.end.line >= line
        {
            log::debug!(
                "match_line name={} start.line={}",
                self.name,
                self.item.range.start.line
            );
            MatchDocumentSymbol::MatchSymbol(self.id, 1)
        } else {
            MatchDocumentSymbol::LineAfterSymbol(1)
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum MatchDocumentSymbol {
    LineBeforeSymbol,
    MatchSymbol(Id, usize),
    LineAfterSymbol(usize),
}
impl MatchDocumentSymbol {
    pub fn is_mach(&self) -> bool {
        matches!(self, MatchDocumentSymbol::MatchSymbol(_, _))
    }

    pub fn is_before(&self) -> bool {
        *self == MatchDocumentSymbol::LineBeforeSymbol
    }

    pub fn line_index(&self) -> Option<usize> {
        if let Self::MatchSymbol(_, line) = self {
            Some(*line)
        } else {
            None
        }
    }
}

fn get_children(
    data: RwSignal<SymbolInformationItemData>,
    next: &mut usize,
    min: usize,
    max: usize,
    level: usize,
    path: Rc<PathBuf>,
) -> Vec<(
    usize,
    usize,
    Rc<PathBuf>,
    RwSignal<SymbolInformationItemData>,
)> {
    let mut children = Vec::new();
    if *next >= min && *next < max {
        children.push((*next, level, path.clone(), data));
    } else if *next >= max {
        return children;
    }
    next.add_assign(1);
    if data.get_untracked().open.get() {
        for child in data.get().children {
            let child_children =
                get_children(child, next, min, max, level + 1, path.clone());
            children.extend(child_children);
            if *next > max {
                break;
            }
        }
    }
    children
}

#[derive(Default, Clone)]
pub struct VirtualList {
    pub root: Option<SymbolData>,
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentSymbolViewData {
    pub virtual_list: RwSignal<VirtualList>,
    pub scroll_to:    RwSignal<Option<f64>>,
    pub select:       RwSignal<Option<Id>>,
}

impl DocumentSymbolViewData {
    pub fn new(cx: Scope) -> Self {
        Self {
            virtual_list: cx.create_rw_signal(VirtualList::default()),
            scroll_to:    cx.create_rw_signal(None),
            select:       cx.create_rw_signal(None),
        }
    }
}
impl VirtualList {
    pub fn new(root: Option<SymbolData>) -> Self {
        Self { root }
    }

    pub fn update(&mut self, root: Option<SymbolData>) {
        self.root = root;
    }

    pub fn match_line_with_children(
        &self,
        line: u32,
    ) -> Option<MatchDocumentSymbol> {
        self.root.as_ref().map(|x| x.match_line_with_children(line))
    }
}

impl
    VirtualVector<(
        usize,
        usize,
        Rc<PathBuf>,
        RwSignal<SymbolInformationItemData>,
    )> for VirtualList
{
    fn total_len(&self) -> usize {
        if let Some(root) = self.root.as_ref() {
            root.file.get_untracked().child_count()
        } else {
            0
        }
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<
        Item = (
            usize,
            usize,
            Rc<PathBuf>,
            RwSignal<SymbolInformationItemData>,
        ),
    > {
        if let Some(root) = self.root.as_ref() {
            let min = range.start;
            let max = range.end;
            let children = root.get_children(min, max);
            children.into_iter()
        } else {
            Vec::new().into_iter()
        }
    }
}

pub fn symbol_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition,
) -> impl View {
    let config = window_tab_data.common.config;
    let ui_line_height = window_tab_data.common.ui_line_height;
    let sync = window_tab_data.common.sync_document_symbol;
    let scroll_rect = window_tab_data.scope.create_rw_signal(Rect::ZERO);
    let window_tab_data_clone = window_tab_data.clone();
    let window_tab_data_sync = window_tab_data.clone();
    h_stack((
        container(common_svg(config, None, LapceIcons::REMOTE)).style(move |x| {
            let (bg, header_height, caret_color) = config.signal(|config| {
                (config.color(LapceColor::PANEL_BACKGROUND),
                 config.ui.header_height.signal(), config.color(LapceColor::LAPCE_BORDER),)
            });
            x.width_full().height(header_height.get() as f64).background(bg.get()).items_center().border_bottom(1.0)
                .border_color(caret_color.get())
        }).on_click_stop(move |_| {
            let Some(sync) = sync.try_update(|x| {
                *x = !*x;
                *x
            }) else {
                return
            };
            if sync {
                let Some(editor) = window_tab_data_sync.main_split.get_active_editor() else {return };
                let offset = editor.cursor().with_untracked(|x| x.offset());
                editor.sync_document_symbol_by_offset(offset);
            }
        }),
        scroll(
            virtual_stack(
                {
                    let window_tab_data = window_tab_data.clone();
                    move || {
                        let editor = window_tab_data.main_split.get_active_editor();
                        editor.map(|x| x.doc().document_symbol_data.virtual_list.get()).unwrap_or_default()
                    }
                },
                move |(_, _, _, item)| item.get_untracked().id,
                move |(_, level, path,  rw_data)| {
                    let data = rw_data.get_untracked();
                    let open = data.open;
                    let has_child = !data.children.is_empty();
                    let kind = data.item.kind;
                    let id = rw_data.get_untracked().id;
                    stack((
                        container(
                            svg(move || {
                                let svg_str = match open.get() {
                                    true => LapceIcons::ITEM_OPENED,
                                    false => LapceIcons::ITEM_CLOSED,
                                };
                                config.with_ui_svg(svg_str)
                            })
                                .style(move |s| {
                                    let (color, size) = config.signal(|config| {
                                        (
                                            config.color(LapceColor::LAPCE_ICON_ACTIVE), config.ui.icon_size.signal()
                                        )
                                    });
                                    let color = if has_child {
                                        color.get()
                                    } else {
                                        Color::TRANSPARENT
                                    };
                                    let size = size.get() as f32;
                                    s.size(size, size)
                                        .color(color)
                                })
                        ).style(|s| s.padding(4.0).margin_left(6.0).margin_right(2.0))
                            .on_click_cont({
                                move |_x| {
                                    if has_child {
                                        open.update(|x| {
                                            *x = !*x;
                                        });
                                    }
                                }
                            }),
                        svg(move || {
                            let (symbol_svg, file_svg) = config.signal(|config| {
                                (config.symbol_svg(kind), config.ui_svg(LapceIcons::FILE))
                            });
                            if let Some(svg) = symbol_svg {
                                svg.get()
                            } else {
                                file_svg.get()
                            }
                        }).style(move |s| {
                            let (caret_color, size, symbol_color) = config.signal(|config| {
                                (
                                    config.color(LapceColor::LAPCE_ICON_ACTIVE), config.ui.icon_size.signal(), config.symbol_color(&kind)
                                )
                            });
                            let size = size.get() as f32;
                            s.min_width(size)
                                .size(size, size)
                                .margin_right(5.0)
                                .color(symbol_color.unwrap_or(caret_color).get())
                        }),
                        label(move || {
                            data.name.replace('\n', "↵")
                        })
                            .style(move |s| {
                                s.selectable(false)
                            }),
                        label(move || {
                            data.detail.clone().unwrap_or_default()
                        }).style(move |s| s.margin_left(6.0)
                            .color(config.with_color(LapceColor::EDITOR_DIM))
                            .selectable(false)
                            .apply_if(
                                data.item.detail.clone().is_none(),
                                |s| s.hide())
                        ),
                    ))
                        .style({
                            let value = window_tab_data.clone();
                            move |s| {
                                s.padding_right(5.0)
                                    .padding_left((level * 10) as f32)
                                    .items_center()
                                    .height(ui_line_height.get())
                                    .hover(|s| {
                                        s.background(
                                            config
                                                .with_color(LapceColor::PANEL_HOVERED_BACKGROUND),
                                        )
                                            .cursor(CursorStyle::Pointer)
                                    }).apply_if(
                                    {
                                        let editor = value.main_split.get_active_editor();
                                        editor.and_then(|x| x.doc().document_symbol_data.select.get().map(|x| x == id)).unwrap_or_default()
                                    },
                                    |x| {
                                        x.background(
                                            config.with_color(
                                                LapceColor::PANEL_CURRENT_BACKGROUND,
                                            ),
                                        )
                                    },
                                )
                            }
                        })
                        .on_click_stop({
                            let window_tab_data = window_tab_data.clone();
                            let data = rw_data;
                            move |_| {
                                let editor = window_tab_data.main_split.get_active_editor();
                                if let Some(x) = editor { x.doc().document_symbol_data.select.set(Some(id)) }
                                let data = data.get_untracked();
                                window_tab_data
                                    .common
                                    .internal_command
                                    .send(InternalCommand::JumpToLocation { location: EditorLocation {
                                        path: path.to_path_buf(),
                                        position: Some(crate::editor::location::EditorPosition::Position(data.item.selection_range.start)),
                                        scroll_offset: None,
                                        ignore_unconfirmed: false,
                                        same_editor_tab: false,
                                    } });
                            }
                        })
                }
                ,
            )
                .style(|s| s.flex_col().absolute().min_width_full()),
        // ).on_resize(move |rect| {
        //     scroll_rect.set(rect);
        // }
        ).on_scroll(move |rect| {
            log::debug!("on_scroll {rect:?}");
            scroll_rect.set(rect);
        }).ensure_visible(move || {
            let editor = window_tab_data_clone.main_split.get_active_editor();
            let scroll_rect = scroll_rect.get_untracked();
            if let Some(line) = editor.and_then(|x| x.doc().document_symbol_data.scroll_to.get()) {
                        let line_height = ui_line_height.get_untracked();
                        let rect = Rect::new(scroll_rect.x0, (line - 3.0).max(0.0) * line_height, scroll_rect.x0, (line + 3.0) * line_height);
                        log::debug!("ensure_visible line={line} {rect:?}");
                        rect
                    } else {
                        log::debug!("ensure_visible scroll_rect {scroll_rect:?}");
                        scroll_rect
                    }
        })
            .style(
                |s| s.flex_grow(1.)
            )
            // .scroll_to({
            //     move || {
            //         let editor = window_tab_data_clone.main_split.get_active_editor();
            //         if let Some(line) = editor.and_then(|x| x.doc().document_symbol_data.scroll_to.get()) {
            //             let line_height = ui_line_height.get_untracked();
            //             Some(
            //                 (
            //                     0.0,
            //                     line * line_height - scroll_rect.get_untracked().height() / 2.0,
            //                 )
            //                     .into(),
            //             )
            //         } else {
            //             None
            //         }
            //     }
            // })
            .debug_name("symbol_panel")
        )).style(move |x| {
        x.width_full().flex_col().height_full().flex_col()
    })
}
