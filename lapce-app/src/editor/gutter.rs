use doc::lines::cursor::CursorAffinity;
use floem::views::editor::phantom_text::{PhantomText, PhantomTextKind};
use floem::{
    context::PaintCx,
    peniko::kurbo::{Point, Rect, Size},
    reactive::{Memo, SignalGet, SignalWith},
    text::{Attrs, AttrsList, FamilyOwned, TextLayout},
    Renderer, View, ViewId,
};
use im::HashMap;
use log::{debug, error};
use lsp_types::Position;
use serde::{Deserialize, Serialize};

use crate::config::{color::LapceColor, LapceConfig};
use crate::doc::Doc;

use super::{view::changes_colors_screen, EditorData};
use anyhow::Result;
pub struct EditorGutterView {
    id: ViewId,
    editor: EditorData,
    width: f64,
    gutter_padding_right: Memo<f32>,
}

pub fn editor_gutter_view(
    editor: EditorData,
    gutter_padding_right: Memo<f32>,
) -> EditorGutterView {
    let id = ViewId::new();

    EditorGutterView {
        id,
        editor,
        width: 0.0,
        gutter_padding_right,
    }
}

impl EditorGutterView {
    fn paint_head_changes(
        &self,
        cx: &mut PaintCx,
        e_data: &EditorData,
        viewport: Rect,
        is_normal: bool,
        config: &LapceConfig,
        doc: &Doc,
    ) -> Result<()> {
        if !is_normal {
            return Ok(());
        }

        let changes = doc.head_changes().get_untracked();
        let line_height = config.editor.line_height() as f64;
        let gutter_padding_right = self.gutter_padding_right.get_untracked() as f64;

        let changes = changes_colors_screen(config, &e_data.editor, changes)?;
        for (y, height, removed, color) in changes {
            let height = if removed {
                10.0
            } else {
                height as f64 * line_height
            };
            let mut y = y - viewport.y0;
            if removed {
                y -= 5.0;
            }
            cx.fill(
                &Size::new(3.0, height).to_rect().with_origin(Point::new(
                    self.width + 5.0 - gutter_padding_right,
                    y,
                )),
                color,
                0.0,
            )
        }
        Ok(())
    }

    fn paint_sticky_headers(
        &self,
        cx: &mut PaintCx,
        is_normal: bool,
        config: &LapceConfig,
    ) {
        if !is_normal {
            return;
        }

        if !config.editor.sticky_header {
            return;
        }
        let sticky_header_height = self.editor.sticky_header_height.get_untracked();
        if sticky_header_height == 0.0 {
            return;
        }

        let sticky_area_rect =
            Size::new(self.width + 25.0 + 30.0, sticky_header_height)
                .to_rect()
                .with_origin(Point::new(-25.0, 0.0))
                .inflate(25.0, 0.0);
        cx.fill(
            &sticky_area_rect,
            config.color(LapceColor::LAPCE_DROPDOWN_SHADOW),
            3.0,
        );
        cx.fill(
            &sticky_area_rect,
            config.color(LapceColor::EDITOR_STICKY_HEADER_BACKGROUND),
            0.0,
        );
    }
}

impl View for EditorGutterView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn compute_layout(
        &mut self,
        _cx: &mut floem::context::ComputeLayoutCx,
    ) -> Option<floem::peniko::kurbo::Rect> {
        if let Some(width) = self.id.get_layout().map(|l| l.size.width as f64) {
            self.width = width;
        }
        None
    }

    fn paint(&mut self, cx: &mut floem::context::PaintCx) {
        let doc = self.editor.doc_signal().get();
        if let Some(path) = doc.content.get_untracked().path() {
            if path.ends_with("test.rs") {
                debug!("{:?}", path);
            }
        }
        let viewport = self.editor.viewport();
        let cursor = self.editor.cursor();
        let screen_lines = doc.lines.with_untracked(|x| x.signal_screen_lines());
        let config = self.editor.common.config;

        let kind_is_normal =
            self.editor.kind().with_untracked(|kind| kind.is_normal());
        let (offset, is_insert) =
            cursor.with_untracked(|c| (c.offset(), c.is_insert()));
        let config = config.get_untracked();
        let line_height = config.editor.line_height() as f64;
        // let _last_line = self.editor.editor.last_line();
        // let current_line = doc
        //     .buffer
        //     .with_untracked(|buffer| buffer.line_of_offset(offset));

        let (current_visual_line, _line_offset, _, _) =
            match doc.lines.with_untracked(|x| {
                x.visual_line_of_offset(offset, CursorAffinity::Forward)
                    .map(|x| (x.0, x.1, x.2, x.3))
            }) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return;
                }
            };

        let family: Vec<FamilyOwned> =
            FamilyOwned::parse_list(&config.editor.font_family).collect();
        let attrs = Attrs::new()
            .family(&family)
            .color(config.color(LapceColor::EDITOR_DIM))
            .font_size(config.editor.font_size() as f32);
        let attrs_list = AttrsList::new(attrs);
        let current_line_attrs_list =
            AttrsList::new(attrs.color(config.color(LapceColor::EDITOR_FOREGROUND)));
        let show_relative = config.core.modal
            && config.editor.modal_mode_relative_line_numbers
            && !is_insert
            && kind_is_normal;

        let current_number = current_visual_line.line_number(false, None);
        screen_lines.with(|screen_lines| {
            for visual_line_info in screen_lines.visual_lines.iter() {
                let line_number = visual_line_info
                    .visual_line
                    .line_number(show_relative, current_number);
                let text_layout = if current_number == line_number {
                    TextLayout::new_with_text(
                        &line_number.map(|x| x.to_string()).unwrap_or_default(),
                        current_line_attrs_list.clone(),
                    )
                } else {
                    TextLayout::new_with_text(
                        &line_number.map(|x| x.to_string()).unwrap_or_default(),
                        attrs_list.clone(),
                    )
                };
                let y = visual_line_info.visual_line_y;
                let size = text_layout.size();
                let height = size.height;

                let x = (self.width
                    - size.width
                    - self.gutter_padding_right.get_untracked() as f64)
                    .max(0.0);
                let y = y + (line_height - height) / 2.0;

                cx.draw_text_with_layout(
                    text_layout.layout_runs(),
                    Point::new(x, y),
                );
            }
        });

        if let Err(err) = self.paint_head_changes(
            cx,
            &self.editor,
            viewport,
            kind_is_normal,
            &config,
            &doc,
        ) {
            error!("{err:?}");
        }
        self.paint_sticky_headers(cx, kind_is_normal, &config);
    }

    fn debug_name(&self) -> std::borrow::Cow<'static, str> {
        "Editor Gutter".into()
    }
}

// fn get_offset(buffer: &Buffer, positon: Position) -> usize {
//     buffer.offset_of_line(positon.line as usize) + positon.character as usize
// }
