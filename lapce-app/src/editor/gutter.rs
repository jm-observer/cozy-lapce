use anyhow::Result;
use doc::lines::{cursor::CursorAffinity, screen_lines::VisualLineInfo};
use floem::{
    Renderer, View, ViewId,
    context::PaintCx,
    peniko::kurbo::{Point, Rect, Size},
    prelude::Color,
    reactive::{Memo, SignalGet, SignalWith},
    text::{Attrs, AttrsList, TextLayout},
};
use log::{debug, error};

use super::{EditorData, view::changes_colors_screen};
use crate::{config::color::LapceColor, doc::Doc};
pub struct EditorGutterView {
    id:                   ViewId,
    editor:               EditorData,
    width:                f64,
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
    #[allow(clippy::too_many_arguments)]
    fn paint_head_changes(
        &self,
        cx: &mut PaintCx,
        e_data: &EditorData,
        viewport: Rect,
        is_normal: bool,
        doc: &Doc,
        added: Color,
        modified_color: Color,
        removed: Color,
        line_height: f64,
    ) -> Result<()> {
        if !is_normal {
            return Ok(());
        }

        let changes = doc.head_changes().get_untracked();
        let gutter_padding_right = self.gutter_padding_right.get_untracked() as f64;

        let changes =
            changes_colors_screen(&e_data, changes, added, modified_color, removed)?;
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
        sticky_header: bool,
        shadow_color: Color,
        hbg: Color,
    ) {
        if !is_normal {
            return;
        }

        if !sticky_header {
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
        cx.fill(&sticky_area_rect, shadow_color, 3.0);
        cx.fill(&sticky_area_rect, hbg, 0.0);
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
        let viewport = self.editor.viewport_untracked();
        let cursor = self.editor.cursor();
        let screen_lines = self.editor.screen_lines;
        // let screen_lines = doc.lines.with_untracked(|x| x.signal_screen_lines());
        let (
            line_height,
            font_family,
            dim,
            font_size,
            fg,
            modal,
            modal_mode_relative_line_numbers,
            shadow,
            header_bg,
            removed,
            modified,
            added,
            sticky_header,
        ) = self.editor.common.config.signal(|config| {
            (
                config.editor.line_height.signal(),
                config.editor.font_family.signal(),
                config.color(LapceColor::EDITOR_DIM),
                config.editor.font_size.signal(),
                config.color(LapceColor::EDITOR_FOREGROUND),
                config.core.modal.signal(),
                config.editor.modal_mode_relative_line_numbers.signal(),
                config.color(LapceColor::LAPCE_DROPDOWN_SHADOW),
                config.color(LapceColor::EDITOR_STICKY_HEADER_BACKGROUND),
                config.color(LapceColor::SOURCE_CONTROL_REMOVED),
                config.color(LapceColor::SOURCE_CONTROL_MODIFIED),
                config.color(LapceColor::SOURCE_CONTROL_ADDED),
                config.editor.sticky_header.signal(),
            )
        });

        let (
            line_height,
            font_family,
            dim,
            font_size,
            fg,
            modal,
            modal_mode_relative_line_numbers,
            shadow,
            header_bg,
            removed,
            modified,
            added,
            sticky_header,
        ) = (
            line_height.get() as f64,
            font_family.get(),
            dim.get(),
            font_size.get() as f32,
            fg.get(),
            modal.get(),
            modal_mode_relative_line_numbers.get(),
            shadow.get(),
            header_bg.get(),
            removed.get(),
            modified.get(),
            added.get(),
            sticky_header.get(),
        );

        let kind_is_normal = self
            .editor
            .kind_read()
            .with_untracked(|kind| kind.is_normal());
        let (offset, is_insert) =
            cursor.with_untracked(|c| (c.offset(), c.is_insert()));

        // let _last_line = self.editor.editor.last_line();
        // let current_line = doc
        //     .buffer
        //     .with_untracked(|buffer| buffer.line_of_offset(offset));

        let (current_visual_line, _line_offset) =
            match doc.lines.with_untracked(|x| {
                x.folded_line_and_final_col_of_offset(
                    offset,
                    CursorAffinity::Forward,
                )
                .map(|x| (x.0.clone(), x.1))
            }) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return;
                },
            };

        let attrs = Attrs::new()
            .family(&font_family.0)
            .color(dim)
            .font_size(font_size);
        let attrs_list = AttrsList::new(attrs);
        let current_line_attrs_list = AttrsList::new(attrs.color(fg));
        let show_relative = modal
            && modal_mode_relative_line_numbers
            && !is_insert
            && kind_is_normal;

        let current_number = current_visual_line.line_number(false, None);
        screen_lines.with_untracked(|screen_lines| {
            for visual_line_info in screen_lines.visual_lines.iter() {
                if let VisualLineInfo::OriginText { text, .. } = visual_line_info {
                    let line_number =
                        text.folded_line.line_number(show_relative, current_number);
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
                    let y = text.folded_line_y;
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
            }
        });

        if let Err(err) = self.paint_head_changes(
            cx,
            &self.editor,
            viewport,
            kind_is_normal,
            &doc,
            added,
            modified,
            removed,
            line_height,
        ) {
            error!("{err:?}");
        }
        self.paint_sticky_headers(
            cx,
            kind_is_normal,
            sticky_header,
            shadow,
            header_bg,
        );
    }

    fn debug_name(&self) -> std::borrow::Cow<'static, str> {
        "Editor Gutter".into()
    }
}

// fn get_offset(buffer: &Buffer, positon: Position) -> usize {
//     buffer.offset_of_line(positon.line as usize) + positon.character as usize
// }
