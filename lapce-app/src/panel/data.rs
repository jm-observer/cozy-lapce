use std::{rc::Rc, sync::Arc};

use floem::{
    kurbo::Size,
    reactive::{
        Memo, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, use_context,
    },
};
use lapce_core::panel::*;

use crate::{
    db::LapceDb,
    window_workspace::{CommonData, Focus},
};

#[derive(Clone)]
pub struct PanelData {
    pub panels:         RwSignal<PanelOrder>,
    pub styles:         RwSignal<im::HashMap<PanelContainerPosition, PanelStyle>>,
    pub size:           RwSignal<PanelSize>,
    pub available_size: Memo<Size>,
    pub sections:       RwSignal<im::HashMap<PanelSection, RwSignal<bool>>>,
    pub common:         Rc<CommonData>,
}

impl PanelData {
    pub fn new(
        cx: Scope,
        panels: im::HashMap<PanelContainerPosition, im::Vector<PanelKind>>,
        available_size: Memo<Size>,
        sections: im::HashMap<PanelSection, bool>,
        common: Rc<CommonData>,
    ) -> Self {
        let panels = cx.create_rw_signal(panels);

        let mut styles = im::HashMap::new();
        styles.insert(
            PanelContainerPosition::Left,
            PanelStyle {
                active:    0,
                shown:     true,
                maximized: false,
            },
        );
        styles.insert(
            PanelContainerPosition::Bottom,
            PanelStyle {
                active:    0,
                shown:     true,
                maximized: false,
            },
        );
        styles.insert(
            PanelContainerPosition::Right,
            PanelStyle {
                active:    0,
                shown:     false,
                maximized: false,
            },
        );
        let styles = cx.create_rw_signal(styles);
        let size = cx.create_rw_signal(PanelSize {
            left:   250.0,
            bottom: 300.0,
            right:  250.0,
        });
        let sections = cx.create_rw_signal(
            sections
                .into_iter()
                .map(|(key, value)| (key, cx.create_rw_signal(value)))
                .collect(),
        );

        Self {
            panels,
            styles,
            size,
            available_size,
            sections,
            common,
        }
    }

    pub fn panel_info(&self) -> PanelInfo {
        PanelInfo {
            panels:   self.panels.get_untracked(),
            styles:   self.styles.get_untracked(),
            size:     self.size.get_untracked(),
            sections: self
                .sections
                .get_untracked()
                .into_iter()
                .map(|(key, value)| (key, value.get_untracked()))
                .collect(),
        }
    }

    pub fn is_container_shown(
        &self,
        position: &PanelContainerPosition,
        tracked: bool,
    ) -> bool {
        self.is_position_shown(position, tracked)
    }

    pub fn is_position_empty(
        &self,
        position: &PanelContainerPosition,
        tracked: bool,
    ) -> bool {
        if tracked {
            self.panels
                .with(|panels| panels.get(position).map(|p| p.is_empty()))
                .unwrap_or(true)
        } else {
            self.panels
                .with_untracked(|panels| panels.get(position).map(|p| p.is_empty()))
                .unwrap_or(true)
        }
    }

    pub fn is_position_shown(
        &self,
        position: &PanelContainerPosition,
        tracked: bool,
    ) -> bool {
        let styles = if tracked {
            self.styles.get()
        } else {
            self.styles.get_untracked()
        };
        styles.get(position).map(|s| s.shown).unwrap_or(false)
    }

    pub fn panel_position(
        &self,
        kind: &PanelKind,
    ) -> Option<(usize, PanelContainerPosition)> {
        self.panels
            .with_untracked(|panels| panel_position(panels, kind))
    }

    pub fn is_panel_visible(&self, kind: &PanelKind) -> bool {
        if let Some((index, position)) = self.panel_position(kind)
            && let Some(style) = self
                .styles
                .with_untracked(|styles| styles.get(&position).cloned())
        {
            return style.active == index && style.shown;
        }
        false
    }

    pub fn show_panel(&self, kind: &PanelKind) {
        if let Some((index, position)) = self.panel_position(kind) {
            self.styles.update(|styles| {
                if let Some(style) = styles.get_mut(&position) {
                    style.shown = true;
                    style.active = index;
                }
            });
        }
    }

    pub fn hide_panel(&self, kind: &PanelKind) {
        if let Some((_, position)) = self.panel_position(kind)
            && let Some((active_panel, _)) =
                self.active_panel_at_position(&position, false)
            && &active_panel == kind
        {
            self.set_shown(&position, false);
        }
    }

    /// Get the active panel kind at that position, if any.  
    /// `tracked` decides whether it should track the signal or not.
    pub fn active_panel_at_position(
        &self,
        position: &PanelContainerPosition,
        tracked: bool,
    ) -> Option<(PanelKind, bool)> {
        let style = if tracked {
            self.styles.with(|styles| styles.get(position).cloned())?
        } else {
            self.styles
                .with_untracked(|styles| styles.get(position).cloned())?
        };
        let order = if tracked {
            self.panels.with(|panels| panels.get(position).cloned())?
        } else {
            self.panels
                .with_untracked(|panels| panels.get(position).cloned())?
        };
        order
            .get(style.active)
            .cloned()
            .or_else(|| order.last().cloned())
            .map(|p| (p, style.shown))
    }

    pub fn set_shown(&self, position: &PanelContainerPosition, shown: bool) {
        self.styles.update(|styles| {
            if let Some(style) = styles.get_mut(position) {
                style.shown = shown;
            }
        });
    }

    pub fn toggle_active_maximize(&self) {
        let focus = self.common.focus.get_untracked();
        if let Focus::Panel(kind) = focus
            && let Some((_, pos)) = self.panel_position(&kind)
            && pos.is_bottom()
        {
            self.toggle_bottom_maximize();
        }
    }

    pub fn toggle_maximize(&self, kind: &PanelKind) {
        if let Some((_, p)) = self.panel_position(kind)
            && p.is_bottom()
        {
            self.toggle_bottom_maximize();
        }
    }

    pub fn toggle_bottom_maximize(&self) {
        let maximized = !self.panel_bottom_maximized(false);
        self.styles.update(|styles| {
            if let Some(style) = styles.get_mut(&PanelContainerPosition::Bottom) {
                style.maximized = maximized;
            }
        });
    }

    pub fn panel_bottom_maximized(&self, tracked: bool) -> bool {
        let styles = if tracked {
            self.styles.get()
        } else {
            self.styles.get_untracked()
        };
        styles
            .get(&PanelContainerPosition::Bottom)
            .map(|p| p.maximized)
            .unwrap_or(false)
    }

    pub fn toggle_container_visual(&self, position: &PanelContainerPosition) {
        let is_hidden = !self.is_container_shown(position, false);
        if is_hidden {
            self.styles.update(|styles| {
                let style = styles.entry(*position).or_default();
                style.shown = true;
            });
        } else {
            if let Some((kind, _)) = self.active_panel_at_position(position, false) {
                self.hide_panel(&kind);
            }
            self.styles.update(|styles| {
                let style = styles.entry(*position).or_default();
                style.shown = false;
            });
        }
    }

    pub fn move_panel_to_position(
        &self,
        kind: PanelKind,
        position: &PanelContainerPosition,
    ) {
        let current_position = self.panel_position(&kind);
        if current_position.as_ref().map(|(_, pos)| pos) == Some(position) {
            return;
        }

        let mut new_index_at_old_position = None;
        let index = self
            .panels
            .try_update(|panels| {
                if let Some((index, current_position)) = current_position
                    && let Some(panels) = panels.get_mut(&current_position)
                {
                    panels.remove(index);

                    let max_index = panels.len().saturating_sub(1);
                    if index > max_index {
                        new_index_at_old_position = Some(max_index);
                    }
                }
                let panels = panels.entry(*position).or_default();
                panels.push_back(kind);
                panels.len() - 1
            })
            .unwrap();
        self.styles.update(|styles| {
            if let Some((_, current_position)) = current_position
                && let Some(new_index) = new_index_at_old_position
            {
                let style = styles.entry(current_position).or_default();
                style.active = new_index;
            }

            let style = styles.entry(*position).or_default();
            style.active = index;
            style.shown = true;
        });

        let db: Arc<LapceDb> = use_context().unwrap();
        db.save_panel_orders(self.panels.get_untracked(), &self.common.local_task);
    }

    pub fn section_open(&self, section: PanelSection) -> RwSignal<bool> {
        let open = self
            .sections
            .with_untracked(|sections| sections.get(&section).cloned());
        if let Some(open) = open {
            return open;
        }

        let open = self.common.scope.create_rw_signal(true);
        self.sections.update(|sections| {
            sections.insert(section, open);
        });
        open
    }
}

pub fn panel_position(
    order: &PanelOrder,
    kind: &PanelKind,
) -> Option<(usize, PanelContainerPosition)> {
    for (pos, panels) in order.iter() {
        let index = panels.iter().position(|k| k == kind);
        if let Some(index) = index {
            return Some((index, *pos));
        }
    }
    None
}
