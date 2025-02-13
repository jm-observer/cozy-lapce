use std::{borrow::Cow, collections::HashMap};

use anyhow::Result;
use floem::text::{Attrs, FamilyOwned, LineHeightValue};
use lapce_xi_rope::Interval;
use log::{debug, error};

use crate::lines::{
    DocLines,
    buffer::rope_text::RopeText,
    delta_compute::{CopyDelta, Offset, OriginLinesDelta},
    line::{OriginFoldedLine, OriginLine, VisualLine}
};

impl DocLines {
    pub fn update_lines_new(
        &mut self,
        mut lines_delta: OriginLinesDelta
    ) -> Result<()> {
        debug!("update_lines_new");
        self.clear();
        self.visual_lines.clear();
        self.line_height = self.config.line_height;

        let all_origin_lines = self.init_all_origin_line_new(&mut lines_delta)?;
        check_origin_lines(&all_origin_lines, self.buffer().len());
        let all_origin_folded_lines =
            self.init_all_origin_folded_line_new(&lines_delta, &all_origin_lines)?;
        {
            let mut visual_line_index = 0;
            // while let Some(line) = origin_line_iter.next() {
            for line in all_origin_folded_lines.iter() {
                // duration += time.elapsed().unwrap();
                let text_layout = &line.text_layout;
                let origin_line_start = text_layout.phantom_text.line;
                let origin_line_end = text_layout.phantom_text.last_line;
                let origin_folded_line_index = line.line_index;

                let origin_interval = Interval {
                    start: self.buffer().offset_of_line(origin_line_start)?,
                    end:   self.buffer().offset_of_line(origin_line_end + 1)?
                };

                let mut visual_offset_start = 0;
                let mut visual_offset_end;

                for (origin_folded_line_sub_index, layout) in
                    text_layout.text.line_layout().iter().enumerate()
                {
                    if layout.glyphs.is_empty() {
                        self.visual_lines.push(VisualLine {
                            line_index:                   visual_line_index,
                            origin_interval:              Interval::new(
                                origin_interval.end,
                                origin_interval.end
                            ),
                            visual_interval:              Interval::new(
                                visual_offset_start,
                                visual_offset_start
                            ),
                            origin_line:                  origin_line_start,
                            origin_folded_line:           origin_folded_line_index,
                            origin_folded_line_sub_index: 0 /* text_layout:
                                                             * text_layout.
                                                             * clone(), */
                        });
                        continue;
                    }
                    visual_offset_end =
                        visual_offset_start + layout.glyphs.len() - 1;
                    let offset_info = text_layout
                        .phantom_text
                        .cursor_position_of_final_col(visual_offset_start);
                    let origin_interval_start =
                        self.buffer().offset_of_line(offset_info.0)? + offset_info.1;
                    let offset_info = text_layout
                        .phantom_text
                        .cursor_position_of_final_col(visual_offset_end);

                    let origin_interval_end =
                        self.buffer().offset_of_line(offset_info.0)? + offset_info.1;
                    let origin_interval = Interval {
                        start: origin_interval_start,
                        end:   origin_interval_end + 1
                    };

                    self.visual_lines.push(VisualLine {
                        line_index: visual_line_index,
                        origin_interval,
                        origin_line: origin_line_start,
                        origin_folded_line: origin_folded_line_index,
                        origin_folded_line_sub_index,
                        // text_layout: text_layout.clone(),
                        visual_interval: Interval::new(
                            visual_offset_start,
                            visual_offset_end + 1
                        )
                    });

                    visual_offset_start = visual_offset_end;
                    visual_line_index += 1;
                }
            }
        }

        self.origin_lines = all_origin_lines;
        self.origin_folded_lines = all_origin_folded_lines;
        self.on_update_lines();
        Ok(())
    }

    pub fn init_all_origin_line_new(
        &self,
        lines_delta: &mut OriginLinesDelta
    ) -> Result<Vec<OriginLine>> {
        let mut origin_lines = Vec::with_capacity(self.buffer().num_lines());
        let last_line = self.buffer().last_line();
        if let CopyDelta::Copy {
            recompute_first_or_last_line: recompute_first_line,
            offset,
            line_offset,
            copy_line
        } = lines_delta.copy_line_start
        {
            if recompute_first_line {
                let line = self.init_origin_line(0)?;
                origin_lines.push(line);
            }
            origin_lines.extend(self.copy_origin_line(
                copy_line,
                offset,
                line_offset
            ));
        }
        let recompute_offset_end = lines_delta.recompute_offset_end;
        let recompute_line_start = lines_delta.recompute_line_start;

        for x in recompute_line_start..=last_line {
            let line = self.init_origin_line(x)?;
            let end = line.start_offset + line.len;
            origin_lines.push(line);
            if end >= recompute_offset_end {
                break;
            }
        }
        if let CopyDelta::Copy {
            recompute_first_or_last_line,
            offset,
            line_offset,
            copy_line
        } = &mut lines_delta.copy_line_end
        {
            let line_offset_new = Offset::new(copy_line.start, origin_lines.len());
            *line_offset = line_offset_new;
            origin_lines.extend(self.copy_origin_line(
                *copy_line,
                *offset,
                line_offset_new
            ));
            if *recompute_first_or_last_line {
                origin_lines.push(self.init_origin_line(last_line)?);
            }
        }
        Ok(origin_lines)
    }

    fn compute_copy_origin_folded_line(
        &self,
        copy_line: Interval,
        offset: Offset,
        line_offset: Offset
    ) -> HashMap<usize, (&OriginFoldedLine, Offset, Offset)> {
        if !copy_line.is_empty() {
            self.origin_folded_lines
                .iter()
                .filter_map(|folded| {
                    if copy_line.start <= folded.origin_line_start
                        && folded.origin_line_end < copy_line.end
                    {
                        let mut origin_line_start = folded.origin_line_start;
                        line_offset.adjust(&mut origin_line_start);
                        Some((origin_line_start, (folded, offset, line_offset)))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            HashMap::new()
        }
    }

    pub(crate) fn init_attrs_without_color<'a>(
        &self,
        family: &'a [FamilyOwned]
    ) -> Attrs<'a> {
        let font_size = self.config.font_size;
        Attrs::new()
            .family(family)
            .font_size(font_size as f32)
            .line_height(LineHeightValue::Px(self.line_height as f32))
    }

    pub(crate) fn init_attrs_with_color<'a>(
        &self,
        family: &'a [FamilyOwned]
    ) -> Attrs<'a> {
        let font_size = self.config.font_size;
        Attrs::new()
            .color(self.editor_style.ed_text_color())
            .family(family)
            .font_size(font_size as f32)
            .line_height(LineHeightValue::Px(self.line_height as f32))
    }

    pub fn init_all_origin_folded_line_new(
        &mut self,
        lines_delta: &OriginLinesDelta,
        all_origin_lines: &[OriginLine]
    ) -> Result<Vec<OriginFoldedLine>> {
        let family =
            Cow::Owned(FamilyOwned::parse_list(&self.config.font_family).collect());
        let attrs = self.init_attrs_with_color(&family);
        let mut origin_folded_lines = Vec::with_capacity(self.buffer().num_lines());
        let mut x = 0;
        let last_line = self.buffer().last_line();
        if let CopyDelta::Copy {
            offset,
            line_offset,
            copy_line,
            ..
        } = lines_delta.copy_line_start
        {
            let last_line = line_offset.adjust_new(copy_line.end);
            let origin_folded_line =
                self.compute_copy_origin_folded_line(copy_line, offset, line_offset);
            while x < last_line {
                let line = if let Some((folded_line, offset, line_offset)) =
                    origin_folded_line.get(&x)
                {
                    folded_line.adjust(
                        *offset,
                        *line_offset,
                        origin_folded_lines.len()
                    )
                } else {
                    self.init_folded_line(
                        x,
                        all_origin_lines,
                        attrs,
                        origin_folded_lines.len()
                    )?
                };
                x = line.origin_line_end + 1;
                origin_folded_lines.push(line);
            }
        }
        let origin_folded_line = if let CopyDelta::Copy {
            offset,
            line_offset,
            copy_line,
            ..
        } = lines_delta.copy_line_start
        {
            self.compute_copy_origin_folded_line(copy_line, offset, line_offset)
        } else {
            HashMap::new()
        };

        while x <= last_line {
            let line = if let Some((folded_line, offset, line_offset)) =
                origin_folded_line.get(&x)
            {
                folded_line.adjust(*offset, *line_offset, origin_folded_lines.len())
            } else {
                self.init_folded_line(
                    x,
                    all_origin_lines,
                    attrs,
                    origin_folded_lines.len()
                )?
            };
            x = line.origin_line_end + 1;
            origin_folded_lines.push(line);
        }
        Ok(origin_folded_lines)
    }

    fn init_folded_line(
        &self,
        current_origin_line: usize,
        all_origin_lines: &[OriginLine],
        attrs: Attrs,
        origin_folded_line_index: usize
    ) -> Result<OriginFoldedLine> {
        let (text_layout, semantic_styles, diagnostic_styles) =
            self.new_text_layout_2(current_origin_line, all_origin_lines, attrs)?;
        // duration += time.elapsed().unwrap();
        let origin_line_start = text_layout.phantom_text.line;
        let origin_line_end = text_layout.phantom_text.last_line;

        let origin_interval = Interval {
            start: self.buffer().offset_of_line(origin_line_start)?,
            end:   self.buffer().offset_of_line(origin_line_end + 1)?
        };

        Ok(OriginFoldedLine {
            line_index: origin_folded_line_index,
            origin_line_start,
            origin_line_end,
            origin_interval,
            text_layout,
            semantic_styles,
            diagnostic_styles
        })
    }

    fn copy_origin_line(
        &self,
        copy_line: Interval,
        offset: Offset,
        line_offset: Offset
    ) -> impl IntoIterator<Item = OriginLine> + '_ {
        self.origin_lines[copy_line.start..copy_line.end]
            .iter()
            .map(move |x| x.adjust(offset, line_offset))
    }

    pub fn check_lines(&self) -> bool {
        check_origin_lines(&self.origin_lines, self.buffer().len())
            && check_origin_folded_lines(
                &self.origin_folded_lines,
                self.buffer().len()
            )
    }
}

pub fn check_origin_lines(origin_lines: &[OriginLine], buffer_len: usize) -> bool {
    let mut offset_line = 0;
    let mut no_error = true;
    for (line, origin_line) in origin_lines.iter().enumerate() {
        if origin_line.line_index != line {
            no_error = false;
            error!(
                "origin_line.line_index={}, but should be {}",
                origin_line.line_index, line
            );
        }
        if origin_line.start_offset != offset_line {
            no_error = false;
            error!(
                "origin_line.start_offset={}, but should be {}",
                origin_line.start_offset, offset_line
            );
        }
        offset_line += origin_line.len;
    }
    if buffer_len != offset_line {
        no_error = false;
        error!(
            "buffer().len={}, but compute result is {}",
            buffer_len, offset_line
        );
    }
    no_error
}

pub fn check_origin_folded_lines(
    origin_folded_lines: &[OriginFoldedLine],
    buffer_len: usize
) -> bool {
    let mut line = 0;
    let mut offset_line = 0;
    let mut no_error = true;
    for (line_index, origin_folded_line) in origin_folded_lines.iter().enumerate() {
        if origin_folded_line.line_index != line_index {
            no_error = false;
            error!(
                "{:?} origin_folded_line.line_index={}, but should be {}",
                origin_folded_line, origin_folded_line.line_index, line_index
            );
        }
        if origin_folded_line.origin_line_start != line {
            no_error = false;
            error!(
                "{:?} origin_folded_line.origin_line_start={}, but should be {}",
                origin_folded_line, origin_folded_line.origin_line_start, line
            );
        }
        if origin_folded_line.text_layout.phantom_text.line != line {
            no_error = false;
            error!(
                "{:?} origin_folded_line.origin_line_start={}, but should be {}",
                origin_folded_line, origin_folded_line.origin_line_start, line
            );
        }
        if origin_folded_line.text_layout.phantom_text.last_line
            != origin_folded_line.origin_line_end
        {
            no_error = false;
            error!(
                "{:?} origin_folded_line.text_layout.phantom_text.last_line={}, \
                 but should be {}",
                origin_folded_line,
                origin_folded_line.text_layout.phantom_text.last_line,
                origin_folded_line.origin_line_end
            );
        }
        if origin_folded_line.origin_interval.start != offset_line {
            no_error = false;
            error!(
                "{:?} origin_folded_line.origin_interval.start={}, but should be {}",
                origin_folded_line,
                origin_folded_line.origin_interval.start,
                offset_line
            );
        }
        if origin_folded_line.origin_interval.start
            != origin_folded_line.text_layout.phantom_text.offset_of_line
        {
            no_error = false;
            error!(
                "{:?} origin_folded_line.origin_interval.start={}, but should be {}",
                origin_folded_line,
                origin_folded_line.origin_interval.start,
                origin_folded_line.text_layout.phantom_text.offset_of_line
            );
        }
        offset_line += origin_folded_line.origin_interval.size();
        line = origin_folded_line.origin_line_end + 1;
    }
    if buffer_len != offset_line {
        error!(
            "buffer().len={}, but compute result is {}",
            buffer_len, offset_line
        );
        no_error = false;
    }
    no_error
}
