use std::{rc::Rc, sync::Arc};

use floem::{
    event::{Event, EventListener, EventPropagation},
    kurbo::Point,
    reactive::{
        create_rw_signal, ReadSignal, RwSignal, SignalGet, SignalUpdate, SignalWith,
    },
    style::{CursorStyle, Style},
    taffy::AlignItems,
    unit::PxPctAuto,
    views::{
        container, dyn_stack, empty, h_stack, label, stack, stack_from_iter, tab,
        text, Decorators,
    },
    AnyView, IntoView, View,
};

use super::{
    debug_view::debug_panel, global_search_view::global_search_panel,
    kind::PanelKind, plugin_view::plugin_panel, position::PanelContainerPosition,
    problem_view::problem_panel, source_control_view::source_control_panel,
    terminal_view::terminal_panel,
};
use crate::panel::data::PanelData;
use crate::{
    app::{clickable_icon, clickable_icon_base},
    config::{color::LapceColor, icon::LapceIcons, LapceConfig},
    file_explorer::view::file_explorer_panel,
    panel::{
        call_hierarchy_view::show_hierarchy_panel, document_symbol::symbol_panel,
        implementation_view::implementation_panel,
        references_view::references_panel,
    },
    window_tab::{DragContent, WindowTabData},
};

pub(crate) const PANEL_PICKER_SIZE: f32 = 40.0;

pub fn foldable_panel_section(
    header: impl View + 'static,
    child: impl View + 'static,
    open: RwSignal<bool>,
    config: ReadSignal<Arc<LapceConfig>>,
) -> impl View {
    stack((
        h_stack((
            clickable_icon_base(
                move || {
                    if open.get() {
                        LapceIcons::PANEL_FOLD_DOWN
                    } else {
                        LapceIcons::PANEL_FOLD_UP
                    }
                },
                None::<Box<dyn Fn()>>,
                || false,
                || false,
                config,
            ),
            header.style(|s| s.align_items(AlignItems::Center).padding_left(3.0)),
        ))
        .style(move |s| {
            s.padding_horiz(10.0)
                .padding_vert(6.0)
                .width_pct(100.0)
                .cursor(CursorStyle::Pointer)
                .background(config.get().color(LapceColor::PANEL_BACKGROUND))
        })
        .on_click_stop(move |_| {
            open.update(|open| *open = !*open);
        }),
        child.style(move |s| s.apply_if(!open.get(), |s| s.hide())),
    ))
}

/// A builder for creating a foldable panel out of sections
pub struct PanelBuilder {
    views: Vec<AnyView>,
    config: ReadSignal<Arc<LapceConfig>>,
    position: PanelContainerPosition,
}
impl PanelBuilder {
    pub fn new(
        config: ReadSignal<Arc<LapceConfig>>,
        position: PanelContainerPosition,
    ) -> Self {
        Self {
            views: Vec::new(),
            config,
            position,
        }
    }

    fn add_general(
        mut self,
        name: &'static str,
        height: Option<PxPctAuto>,
        view: impl View + 'static,
        open: RwSignal<bool>,
        style: impl Fn(Style) -> Style + 'static,
    ) -> Self {
        let position = self.position;
        let view = foldable_panel_section(
            text(name).style(move |s| s.selectable(false)),
            view,
            open,
            self.config,
        )
        .style(move |s| {
            let s = s.width_full().flex_col();
            // Use the manual height if given, otherwise if we're open behave flex,
            // otherwise, do nothing so that there's no height
            let s = if open.get() {
                if let Some(height) = height {
                    s.height(height)
                } else {
                    s.flex_grow(1.0).flex_basis(0.0)
                }
            } else if position.is_bottom() {
                s.flex_grow(0.3).flex_basis(0.0)
            } else {
                s
            };

            style(s)
        });
        self.views.push(view.into_any());
        self
    }

    /// Add a view to the panel
    pub fn add(
        self,
        name: &'static str,
        view: impl View + 'static,
        open: RwSignal<bool>,
    ) -> Self {
        self.add_general(name, None, view, open, std::convert::identity)
    }

    /// Add a view to the panel with a custom style applied to the overall header+section-content
    pub fn add_style(
        self,
        name: &'static str,
        view: impl View + 'static,
        open: RwSignal<bool>,
        style: impl Fn(Style) -> Style + 'static,
    ) -> Self {
        self.add_general(name, None, view, open, style)
    }

    /// Add a view to the panel with a custom height that is only used when the panel is open
    pub fn add_height(
        self,
        name: &'static str,
        height: impl Into<PxPctAuto>,
        view: impl View + 'static,
        open: RwSignal<bool>,
    ) -> Self {
        self.add_general(
            name,
            Some(height.into()),
            view,
            open,
            std::convert::identity,
        )
    }

    /// Add a view to the panel with a custom height that is only used when the panel is open
    /// and a custom style applied to the overall header+section-content
    pub fn add_height_style(
        self,
        name: &'static str,
        height: impl Into<PxPctAuto>,
        view: impl View + 'static,
        open: RwSignal<bool>,
        style: impl Fn(Style) -> Style + 'static,
    ) -> Self {
        self.add_general(name, Some(height.into()), view, open, style)
    }

    /// Add a view to the panel with a custom height that is only used when the panel is open
    pub fn add_height_pct(
        self,
        name: &'static str,
        height: f64,
        view: impl View + 'static,
        open: RwSignal<bool>,
    ) -> Self {
        self.add_general(
            name,
            Some(PxPctAuto::Pct(height)),
            view,
            open,
            std::convert::identity,
        )
    }

    /// Build the panel into a view
    pub fn build(self) -> impl View {
        stack_from_iter(self.views).style(move |s| {
            s.width_full()
                .apply_if(!self.position.is_bottom(), |s| s.flex_col())
        })
    }
}

pub fn new_left_panel_container_view(
    window_tab_data: Rc<WindowTabData>,
    position: PanelContainerPosition,
) -> impl View {
    let panel = window_tab_data.panel.clone();
    let config = window_tab_data.common.config;
    let dragging = window_tab_data.common.dragging;
    let panel = panel.clone();
    let panels = move || {
        panel
            .panels
            .with(|p| p.get(&position).cloned().unwrap_or_default())
    };
    let active_fn = move || {
        panel
            .styles
            .with(|s| s.get(&position).map(|s| s.active).unwrap_or(0))
    };
    let window_tab_data = window_tab_data.clone();
    stack((
        tab(
            active_fn,
            panels,
            |p| *p,
            move |kind| panel_view_by_kind(kind, window_tab_data.clone(), position),
        )
        .style(|s| s.flex_grow(1.0).height_pct(100.0)),
        drag_line(position, panel.clone(), config),
    ))
    .style({
        let panel = panel.clone();
        move |s| {
            s.flex_row()
                .width(panel.size.get().left as f32)
                .height_pct(100.0)
                .border_color(config.get().color(LapceColor::LAPCE_BORDER))
                .border_right(1.0)
                .apply_if(
                    !panel.is_position_shown(&position, true)
                        || panel.is_position_empty(&position, true),
                    |s| s.hide(),
                )
        }
    })
    .debug_name("panel left view")
    // .style({
    //     move |s| {
    //         let config = config.get();
    //         s.flex_row()
    //             .height_pct(100.0)
    //             .background(config.color(LapceColor::PANEL_BACKGROUND))
    //             .color(config.color(LapceColor::PANEL_FOREGROUND))
    //     }
    // })
    // .debug_name(position.debug_name())
    .event(move |x| drag_event(x, config, dragging, panel.clone(), position))
}

pub fn new_bottom_panel_container_view(
    window_tab_data: Rc<WindowTabData>,
    position: PanelContainerPosition,
) -> impl View {
    let panel = window_tab_data.panel.clone();
    let config = window_tab_data.common.config;
    let dragging = window_tab_data.common.dragging;
    // stack((
    let panel = panel.clone();
    let panels = move || {
        panel
            .panels
            .with(|p| p.get(&position).cloned().unwrap_or_default())
    };
    let active_fn = move || {
        panel
            .styles
            .with(|s| s.get(&position).map(|s| s.active).unwrap_or(0))
    };
    let window_tab_data = window_tab_data.clone();
    stack((
        drag_line(position, panel.clone(), config),
        tab(
            active_fn,
            panels,
            |p| *p,
            move |kind| panel_view_by_kind(kind, window_tab_data.clone(), position),
        )
        .style(|s| s.flex_grow(1.0).width_pct(100.0)),
    ))
    .style({
        let panel = panel.clone();
        move |s| {
            s.flex_col()
                .height(panel.size.get().bottom as f32)
                .width_pct(100.0)
                .border_color(config.get().color(LapceColor::LAPCE_BORDER))
                .border_top(1.0)
                .apply_if(
                    !panel.is_position_shown(&position, true)
                        || panel.is_position_empty(&position, true),
                    |s| s.hide(),
                )
        }
    })
    .debug_name("panel bottom view")
    .event(move |x| drag_event(x, config, dragging, panel.clone(), position))
}

pub fn new_right_panel_container_view(
    window_tab_data: Rc<WindowTabData>,
    container_position: PanelContainerPosition,
) -> impl View {
    let panel = window_tab_data.panel.clone();
    let config = window_tab_data.common.config;
    let dragging = window_tab_data.common.dragging;
    let position = container_position;
    let panel = panel.clone();
    {
        let panel = window_tab_data.panel.clone();
        let config = window_tab_data.common.config;
        let panels = move || {
            panel
                .panels
                .with(|p| p.get(&position).cloned().unwrap_or_default())
        };
        let active_fn = move || {
            panel
                .styles
                .with(|s| s.get(&position).map(|s| s.active).unwrap_or(0))
        };
        stack((
            drag_line(position, panel.clone(), config),
            tab(active_fn, panels, |p| *p, {
                let window_tab_data = window_tab_data.clone();
                move |kind| {
                    panel_view_by_kind(kind, window_tab_data.clone(), position)
                }
            })
            .style(|s| s.flex_grow(1.0).height_pct(100.0)),
        ))
        .style(move |s| s.flex_row().width(panel.size.get().right as f32))
    }
    .style({
        let panel = panel.clone();
        move |s| {
            s.flex_grow(1.0)
                .height_pct(100.0)
                .border_left(1.0)
                .border_color(config.get().color(LapceColor::LAPCE_BORDER))
                .apply_if(
                    !panel.is_position_shown(&position, true)
                        || panel.is_position_empty(&position, true),
                    |s| s.hide(),
                )
        }
    })
    .debug_name("panel right view")
    // .style({
    //     move |s| {
    //         let config = config.get();
    //         s.flex_row()
    //             .margin_left(Auto)
    //             .height_pct(100.0)
    //             .background(config.color(LapceColor::PANEL_BACKGROUND))
    //             .color(config.color(LapceColor::PANEL_FOREGROUND))
    //     }
    // })
    // .debug_name(container_position.debug_name())
    .event(move |x| drag_event(x, config, dragging, panel.clone(), position))
}

fn panel_view_by_kind(
    kind: PanelKind,
    window_tab_data: Rc<WindowTabData>,
    position: PanelContainerPosition,
) -> impl View {
    match kind {
        PanelKind::Terminal => terminal_panel(window_tab_data.clone()).into_any(),
        PanelKind::FileExplorer => {
            file_explorer_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::SourceControl => {
            source_control_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::Plugin => {
            plugin_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::Search => {
            global_search_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::Problem => {
            problem_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::Debug => {
            debug_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::CallHierarchy => {
            show_hierarchy_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::DocumentSymbol => {
            symbol_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::References => {
            references_panel(window_tab_data.clone(), position).into_any()
        }
        PanelKind::Implementation => {
            implementation_panel(window_tab_data.clone(), position).into_any()
        }
    }
}

pub fn panel_header(
    header: String,
    config: ReadSignal<Arc<LapceConfig>>,
) -> impl View {
    container(label(move || header.clone())).style(move |s| {
        s.padding_horiz(10.0)
            .padding_vert(6.0)
            .width_pct(100.0)
            .background(config.get().color(LapceColor::EDITOR_BACKGROUND))
    })
}

fn drag_line(
    position: PanelContainerPosition,
    panel: PanelData,
    config: ReadSignal<Arc<LapceConfig>>,
) -> impl View {
    let panel_size = panel.size;
    let view = empty();
    let view_id = view.id();
    let drag_start: RwSignal<Option<Point>> = create_rw_signal(None);
    view.on_event_stop(EventListener::PointerDown, move |event| {
        view_id.request_active();
        if let Event::PointerDown(pointer_event) = event {
            drag_start.set(Some(pointer_event.pos));
        }
    })
    .on_event_stop(EventListener::PointerMove, move |event| {
        if let Event::PointerMove(pointer_event) = event {
            if drag_start.get_untracked().is_some() {
                // log::info!("pos.y = {}", pointer_event.pos.y);
                match position {
                    PanelContainerPosition::Left => {
                        let current_panel_size = panel_size.get_untracked();
                        let new_size = pointer_event.pos.x + current_panel_size.left;
                        let new_size = new_size.max(150.0);
                        if new_size != current_panel_size.left {
                            panel_size.update(|size| {
                                size.left = new_size;
                            })
                        }
                    }
                    PanelContainerPosition::Bottom => {
                        let current_panel_size = panel_size.get_untracked();
                        let new_size =
                            current_panel_size.bottom - pointer_event.pos.y;
                        // log::info!(
                        //     "new_size={} pointer_event.pos.y={}",
                        //     new_size,
                        //     pointer_event.pos.y
                        // );
                        let new_size = new_size.max(60.0);
                        if new_size != current_panel_size.bottom {
                            panel_size.update(|size| {
                                size.bottom = new_size;
                            })
                        }
                    }
                    PanelContainerPosition::Right => {
                        let current_panel_size = panel_size.get_untracked();
                        let new_size =
                            current_panel_size.right - pointer_event.pos.x;
                        let new_size = new_size.max(150.0);
                        if new_size != current_panel_size.right {
                            panel_size.update(|size| {
                                size.right = new_size;
                            })
                        }
                    }
                }
            }
        }
    })
    .on_event_stop(EventListener::PointerUp, move |_| {
        drag_start.set(None);
    })
    .style(move |s| {
        let is_dragging = drag_start.get().is_some();
        let config = config.get();
        s.background(config.color(LapceColor::PANEL_BACKGROUND))
            .apply_if(position == PanelContainerPosition::Bottom, |s| {
                s.width_pct(100.0).height(4.0)
            })
            .apply_if(
                position == PanelContainerPosition::Left
                    || position == PanelContainerPosition::Right,
                |s| s.width(4.0).height_pct(100.0),
            )
            .apply_if(is_dragging, |s| {
                s.background(config.color(LapceColor::EDITOR_CARET))
                    .apply_if(position == PanelContainerPosition::Bottom, |s| {
                        s.cursor(CursorStyle::RowResize)
                    })
                    .apply_if(position != PanelContainerPosition::Bottom, |s| {
                        s.cursor(CursorStyle::ColResize)
                    })
                    .z_index(2)
            })
            .hover(|s| {
                s.background(config.color(LapceColor::EDITOR_CARET))
                    .apply_if(position == PanelContainerPosition::Bottom, |s| {
                        s.cursor(CursorStyle::RowResize)
                    })
                    .apply_if(position != PanelContainerPosition::Bottom, |s| {
                        s.cursor(CursorStyle::ColResize)
                    })
                    .z_index(2)
            })
    })
}

pub(crate) fn new_panel_picker(
    window_tab_data: Rc<WindowTabData>,
    position: PanelContainerPosition,
) -> impl View {
    let panel = window_tab_data.panel.clone();
    let config = window_tab_data.common.config;
    let dragging = window_tab_data.common.dragging;
    let is_bottom = position.is_bottom();
    dyn_stack(
        move || {
            panel
                .panels
                .with(|panels| panels.get(&position).cloned().unwrap_or_default())
        },
        |p| *p,
        move |p| {
            let window_tab_data = window_tab_data.clone();
            let tooltip = p.tooltip();
            let icon = p.svg_name();
            let is_active = {
                let window_tab_data = window_tab_data.clone();
                move || {
                    if let Some((active_panel, shown)) = window_tab_data
                        .panel
                        .active_panel_at_position(&position, true)
                    {
                        shown && active_panel == p
                    } else {
                        false
                    }
                }
            };
            container(stack((
                clickable_icon(
                    || icon,
                    move || {
                        window_tab_data.toggle_panel_visual(p);
                    },
                    || false,
                    || false,
                    move || tooltip,
                    config,
                )
                .draggable()
                .on_event_stop(EventListener::DragStart, move |_| {
                    dragging.set(Some(DragContent::Panel(p)));
                })
                .on_event_stop(EventListener::DragEnd, move |_| {
                    dragging.set(None);
                })
                .dragging_style(move |s| {
                    let config = config.get();
                    s.border(1.0)
                        .border_radius(6.0)
                        .border_color(config.color(LapceColor::LAPCE_BORDER))
                        .padding(6.0)
                        .background(
                            config
                                .color(LapceColor::PANEL_BACKGROUND)
                                .multiply_alpha(0.7),
                        )
                })
                .style(|s| s.padding(1.0)),
                label(|| "".to_string()).style(move |s| {
                    s.selectable(false)
                        .absolute()
                        .size_pct(100.0, 100.0)
                        .apply_if(!is_bottom, |s| s.margin_top(2.0))
                        .apply_if(is_bottom, |s| s.margin_left(-2.0))
                        .apply_if(is_active(), |s| {
                            s.apply_if(position.is_left(), |s| s.border_left(2.0))
                                .apply_if(is_bottom, |s| s.border_bottom(2.0))
                                .apply_if(position.is_right(), |s| {
                                    s.border_right(2.0)
                                })
                        })
                        .border_color(
                            config
                                .get()
                                .color(LapceColor::LAPCE_TAB_ACTIVE_UNDERLINE),
                        )
                }),
            )))
            .style(|s| s.padding(4.0))
        },
    )
    .style(move |s| {
        s.flex_row()
            .padding(1.0)
            .background(config.get().color(LapceColor::PANEL_BACKGROUND))
    })
    .event(move |x| drag_event(x, config, dragging, panel.clone(), position))
}

fn drag_event<T: IntoView>(
    view: T,
    config: ReadSignal<Arc<LapceConfig>>,
    dragging: RwSignal<Option<DragContent>>,
    panel: PanelData,
    position: PanelContainerPosition,
) -> <T as IntoView>::V {
    let panel = panel.clone();

    let is_dragging_panel = move || {
        dragging
            .with_untracked(|d| d.as_ref().map(|d| d.is_panel()))
            .unwrap_or(false)
    };

    let dragging_over = create_rw_signal(false);
    view.on_event(EventListener::DragEnter, move |_| {
        if is_dragging_panel() {
            dragging_over.set(true);
            EventPropagation::Stop
        } else {
            EventPropagation::Continue
        }
    })
    .on_event(EventListener::DragLeave, move |_| {
        if is_dragging_panel() {
            dragging_over.set(false);
            EventPropagation::Stop
        } else {
            EventPropagation::Continue
        }
    })
    .on_event(EventListener::Drop, move |_| {
        if let Some(DragContent::Panel(kind)) = dragging.get_untracked() {
            dragging_over.set(false);
            panel.move_panel_to_position(kind, &position);
            EventPropagation::Stop
        } else {
            EventPropagation::Continue
        }
    })
    .style(move |s| {
        s.apply_if(dragging_over.get(), |s| {
            s.background(config.get().color(LapceColor::EDITOR_DRAG_DROP_BACKGROUND))
        })
    })
}

trait OnEvent: IntoView<V = Self::DV> + Sized {
    type DV: View;
    fn event(self, on_event: impl Fn(Self) -> Self::DV + 'static) -> Self::DV {
        on_event(self)
    }
}

impl<T: IntoView + Sized> OnEvent for T {
    type DV = <T as IntoView>::V;
    fn event(self, on_event: impl Fn(Self) -> Self::DV + 'static) -> Self::DV {
        on_event(self)
    }
}
