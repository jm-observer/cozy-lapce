use std::{
    cell::RefCell,
    future::Future,
    ops::{AddAssign, Range},
    thread
};

use ansi_to_style::TextStyle;
use anyhow::Result;
use doc::lines::layout::*;
use floem::{
    ViewId,
    kurbo::Point,
    peniko::Color,
    prelude::{RwSignal, SignalGet, SignalUpdate, VirtualVector},
    reactive::{Scope, batch}
};
use lapce_xi_rope::Rope;
use log::error;

use crate::{
    channel::{ExtChannel, create_signal_from_channel},
    views::panel::{DisplayId, DocManager, DocStyle, Hyperlink, TextSrc}
};

pub fn ranges_overlap(r1: &Range<usize>, r2: &Range<usize>) -> Option<Range<usize>> {
    let overlap = if r2.start <= r1.start && r1.start < r2.end {
        r1.start..r1.end.min(r2.end)
    } else if r1.start <= r2.start && r2.start < r1.end {
        r2.start..r2.end.min(r1.end)
    } else {
        return None;
    };
    if overlap.is_empty() {
        None
    } else {
        Some(overlap)
    }
}

#[derive(Clone)]
pub struct TreePanelData {
    pub cx:         Scope,
    pub node:       RwSignal<TreeNode>,
    pub doc:        DocManager,
    pub left_width: RwSignal<f64>
}

impl TreePanelData {
    pub fn new(cx: Scope, doc_style: DocStyle) -> Self {
        let doc = DocManager::new(cx, ViewId::new(), doc_style);
        let node = cx.create_rw_signal(TreeNode {
            display_id: DisplayId::All,
            cx,
            children: vec![],
            open: cx.create_rw_signal(true),
            level: cx.create_rw_signal(Level::None)
        });
        let left_width = cx.create_rw_signal(200.0);
        Self {
            cx,
            node,
            doc,
            left_width
        }
    }

    pub fn run_with_async_task<F, Fut>(&self, f: F)
    where
        F: Fn(ExtChannel<crate::views::tree_with_panel::data::StyledText>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = anyhow::Result<()>> {
        let (read_signal, channel, send) =
            create_signal_from_channel::<StyledText>(self.cx);
        let data = self.clone();
        self.cx.create_effect(move |_| {
            if let Some(line) = read_signal.get() {
                data.node
                    .update(|x| x.add_child(line.id.display_id(), line.level));
                data.doc.update(|x| {
                    if let Err(err) = x.append_lines(line) {
                        error!("{err:?}");
                    }
                });
            }
        });
        thread::spawn(|| {
            async_main_run(channel, f);
            send(())
        });
    }

    pub fn run_with_sync_task<F>(&self, f: F)
    where
        F: Fn(
                ExtChannel<crate::views::tree_with_panel::data::StyledText>
            ) -> anyhow::Result<()>
            + Sync
            + Send
            + 'static {
        let (read_signal, channel, send) =
            create_signal_from_channel::<StyledText>(self.cx);
        let data = self.clone();
        self.cx.create_effect(move |_| {
            if let Some(line) = read_signal.get() {
                data.node
                    .update(|x| x.add_child(line.id.display_id(), line.level));
                data.doc.update(|x| {
                    if let Err(err) = x.append_lines(line) {
                        error!("{err:?}");
                    }
                });
            }
        });
        thread::spawn(move || {
            if let Err(err) = f(channel) {
                error!("{err:?}");
            }
            send(())
        });
    }
}

#[tokio::main(flavor = "current_thread")]
async fn async_main_run<F, Fut>(channel: ExtChannel<StyledText>, f: F)
where
    F: Fn(ExtChannel<StyledText>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>> {
    if let Err(err) = f(channel).await {
        error!("{:?}", err);
    }
}

#[derive(Clone)]
pub struct StyledLines {
    pub text_src: TextSrc,
    pub lines:    Vec<(String, Vec<TextStyle>, Vec<Hyperlink>)>
}

#[derive(Clone, Debug)]
pub struct VisualLine {
    pub pos_y:      f64,
    pub line_index: usize,
    pub hyperlinks: Vec<(Point, Point, Color)>,
    pub text:       RefCell<TextLayout>
}

#[derive(Clone)]
pub struct StyledText {
    pub id:          TextSrc,
    pub level:       Level,
    pub styled_text: ansi_to_style::TextWithStyle,
    pub hyperlink:   Vec<Hyperlink>
}

impl StyledText {
    pub fn to_lines(self) -> Result<StyledLines> {
        let rope: Rope = self.styled_text.text.into();
        let last_line = rope.line_of_offset(rope.len()) + 1;
        // if last_line > 1 {
        //     error!("last_line={} {} {:?} {:?}", last_line,
        // rope.to_string(), self.styled_text.styles, self.hyperlink)
        // }
        let trim_str = ['\r', '\n'];
        //styles: Vec<(String, Vec<TextStyle>, Vec<Hyperlink>)>,
        let mut lines = Vec::with_capacity(last_line);
        for line in 0..last_line {
            let start_offset = rope.offset_of_line(line)?;
            let end_offset = rope.offset_of_line(line + 1)?;

            let content_origin = rope.slice_to_cow(start_offset..end_offset);
            let content = content_origin.trim_end_matches(trim_str);
            if start_offset == end_offset || content.is_empty() {
                continue;
            }
            let range = start_offset..start_offset + content.len();
            let links = self
                .hyperlink
                .iter()
                .filter_map(|x| {
                    if let Some(delta_range) = ranges_overlap(&x.range(), &range) {
                        let mut link = x.clone();
                        let delta_range = delta_range.start - start_offset
                            ..delta_range.end - start_offset;
                        link.range_mut(delta_range);
                        Some(link)
                    } else {
                        None
                    }
                })
                .collect();

            let styles = self
                .styled_text
                .styles
                .iter()
                .filter_map(|x| {
                    ranges_overlap(&x.range, &range).map(|delta_range| TextStyle {
                        range:     delta_range.start - start_offset
                            ..delta_range.end - start_offset,
                        bold:      x.bold,
                        italic:    x.italic,
                        underline: x.underline,
                        bg_color:  x.bg_color,
                        fg_color:  x.fg_color
                    })
                })
                .collect();
            // if last_line > 1 {
            //     error!("[{content}] {:?} {:?}", styles, links)
            // }

            lines.push((content.to_string(), styles, links));
        }
        Ok(StyledLines {
            text_src: self.id,
            lines
        })
    }
}

#[derive(Clone)]
pub struct TreeNode {
    pub cx:         Scope,
    pub display_id: DisplayId,
    pub children:   Vec<TreeNode>,
    pub level:      RwSignal<Level>,
    pub open:       RwSignal<bool>
}

#[derive(Clone, Debug)]
pub struct TreeNodeData {
    pub display_id: DisplayId,
    pub open:       RwSignal<bool>,
    pub level:      RwSignal<Level>
}

#[derive(Clone, Debug, Copy)]
#[repr(u8)]
pub enum Level {
    None,
    Warn,
    Error
}

impl Level {
    pub fn update(&mut self, level: Level) {
        // debug!("{:?} {level:?}={}", self, level as u8);
        if level as u8 > *self as u8 {
            *self = level
        }
        // debug!("after {:?}", self);
        // use Level::*;
        // if matches!(self, ref level) {
        //     return;
        // }
        // let new_level = match (level, &self) {
        //     (Error, Warn) | (_, None) => level,
        //      _ => return,
        // };
        // *self = new_level;
    }
}

impl TreeNodeData {
    pub fn track_level_svg(&self) -> &'static str {
        match self.level.get() {
            Level::None => {
                // empty.svg
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"></svg>"#
            },
            Level::Warn | Level::Error => {
                // warning.svg
                r#"<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M7.56 1h.88l6.54 12.26-.44.74H1.44L1 13.26 7.56 1zM8 2.28L2.28 13H13.7L8 2.28zM8.625 12v-1h-1.25v1h1.25zm-1.25-2V6h1.25v4h-1.25z"/></svg>"#
            }
        }
    }

    pub fn track_level_svg_color(&self) -> Option<Color> {
        match self.level.get() {
            Level::None => None,
            Level::Warn => Some(Color::from_rgb8(255, 204, 102)),
            Level::Error => Some(Color::from_rgb8(255, 153, 153))
        }
    }
}

impl TreeNode {
    pub fn add_child(&mut self, id: DisplayId, level: Level) {
        batch(|| self._add_child(id, level));
    }

    fn _add_child(&mut self, id: DisplayId, level: Level) {
        // debug!("add_child {:?}", id);
        match &id {
            DisplayId::All => {},
            DisplayId::Error | DisplayId::Crate { .. } => {
                self.level.update(|x| x.update(level));
                if let Some(item) =
                    self.children.iter_mut().find(|x| id == x.display_id)
                {
                    item.level.update(|x| x.update(level));
                } else {
                    self.children.push(TreeNode {
                        cx:         self.cx,
                        display_id: id,
                        level:      self.cx.create_rw_signal(level),
                        open:       self.cx.create_rw_signal(true),
                        children:   vec![]
                    })
                }
            },
            DisplayId::CrateFile { crate_name, .. } => {
                let crate_id = DisplayId::Crate {
                    crate_name: crate_name.clone()
                };
                self.add_child(crate_id.clone(), level);
                if let Some(carte_item) =
                    self.children.iter_mut().find(|x| crate_id == x.display_id)
                {
                    if let Some(item) =
                        carte_item.children.iter_mut().find(|x| id == x.display_id)
                    {
                        item.level.update(|x| x.update(level));
                    } else {
                        carte_item.children.push(TreeNode {
                            cx:         self.cx,
                            display_id: id,
                            level:      self.cx.create_rw_signal(level),
                            open:       self.cx.create_rw_signal(false),
                            children:   vec![]
                        })
                    }
                }
            }
        }
    }

    fn to_data(&self) -> TreeNodeData {
        TreeNodeData {
            display_id: self.display_id.clone(),
            open:       self.open,
            level:      self.level
        }
    }

    fn total(&self) -> usize {
        if self.open.get() {
            self.children.iter().fold(1, |mut total, x| {
                total += x.total();
                total
            })
        } else {
            1
        }
    }

    fn get_children(
        &self,
        min: usize,
        max: usize,
        index: &mut usize,
        level: usize
    ) -> Vec<(usize, usize, TreeNodeData)> {
        let mut children_data = Vec::new();
        if min <= *index && *index <= max {
            children_data.push((*index, level, self.to_data()));
        } else {
            return children_data;
        }
        index.add_assign(1);
        if self.open.get() {
            for child in self.children.iter() {
                let mut children = child.get_children(min, max, index, level + 1);
                children_data.append(&mut children);
            }
        }
        children_data
    }
}

impl VirtualVector<(usize, usize, TreeNodeData)> for TreeNode {
    fn total_len(&self) -> usize {
        self.total()
    }

    fn slice(
        &mut self,
        range: std::ops::Range<usize>
    ) -> impl Iterator<Item = (usize, usize, TreeNodeData)> {
        let min = range.start;
        let max = range.end;
        let mut index = 0;
        let children = self.get_children(min, max, &mut index, 0);
        // debug!("min={min} max={max} {:?}", children);
        children.into_iter()
    }
}
