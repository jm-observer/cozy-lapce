use anyhow::{Result, anyhow};
use doc::lines::{
    EditBuffer,
    buffer::rope_text::RopeText,
    command::EditCommand,
    delta_compute::{
        CopyDelta, Offset, OffsetDelta, OriginLinesDelta, resolve_delta_compute,
        resolve_line_delta
    },
    register::Register
};
use lapce_xi_rope::{DeltaElement, Interval, RopeDelta, RopeInfo, tree::Node};

use crate::lines_util::{cursor_insert, init_main_2};

mod lines_util;

#[test]
fn test_do_insert() -> anyhow::Result<()> {
    custom_utils::logger::logger_stdout_debug();

    {
        let mut lines = init_main_2()?;
        let mut cursor = cursor_insert(117, 117);
        let s = "m";
        let mut response = Vec::new();
        let edit = EditBuffer::DoInsertBuffer {
            cursor:   &mut cursor,
            s,
            response: &mut response
        };
        let _ = lines.buffer_edit(edit).unwrap();
        if !lines.check_lines() {
            lines.log();
        }
    }

    Ok(())
}

#[test]
fn test_insert_new_line() -> anyhow::Result<()> {
    custom_utils::logger::logger_stdout_debug();
    {
        let mut lines = init_main_2()?;
        let mut cursor = cursor_insert(461, 461);
        let cmd = EditCommand::InsertNewLine;
        let mut register = Register::default();
        let smart_tab = true;
        let mut response = Vec::new();

        let edit = EditBuffer::DoEditBuffer {
            cursor: &mut cursor,
            cmd: &cmd,
            modal: false,
            register: &mut register,
            smart_tab,
            response: &mut response
        };
        let _ = lines.buffer_edit(edit).unwrap();
        if !lines.check_lines() {
            lines.log();
            return Ok(());
        }
    }
    {
        let mut lines = init_main_2()?;
        let mut cursor = cursor_insert(0, 0);
        let cmd = EditCommand::InsertNewLine;
        let mut register = Register::default();
        let smart_tab = true;
        let mut response = Vec::new();

        let edit = EditBuffer::DoEditBuffer {
            cursor: &mut cursor,
            cmd: &cmd,
            modal: false,
            register: &mut register,
            smart_tab,
            response: &mut response
        };
        let _ = lines.buffer_edit(edit).unwrap();
        if !lines.check_lines() {
            lines.log();
            return Ok(());
        }
    }
    {
        let mut lines = init_main_2()?;
        let mut cursor = cursor_insert(117, 117);
        let cmd = EditCommand::InsertNewLine;
        let mut register = Register::default();
        let smart_tab = true;
        let mut response = Vec::new();

        let edit = EditBuffer::DoEditBuffer {
            cursor: &mut cursor,
            cmd: &cmd,
            modal: false,
            register: &mut register,
            smart_tab,
            response: &mut response
        };
        let _ = lines.buffer_edit(edit).unwrap();
        if !lines.check_lines() {
            lines.log();
        }
    }

    Ok(())
}

#[test]
fn test_resolve_delta_compute() -> Result<()> {
    let lines = init_main_2()?;
    {
        let rope_delta = RopeDelta {
            els:      vec![
                DeltaElement::<RopeInfo>::Copy(0, 461),
                DeltaElement::<RopeInfo>::Insert(Node::from_leaf(
                    "\r\n".to_string()
                )),
            ],
            base_len: 461
        };
        let offset =
            resolve_delta_compute(&rope_delta).ok_or(anyhow!("rs is none"))?;
        assert_eq!(
            offset,
            OffsetDelta {
                copy_start:   Interval::new(0, 461),
                internal_len: 2,
                copy_end:     Interval::new(0, 0)
            }
        );
        let rs = resolve_line_delta(lines.buffer().text(), offset)?;
        assert_eq!(
            rs,
            OriginLinesDelta {
                copy_line_start:      CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Default::default(),
                    line_offset: Default::default(),
                    copy_line: Interval::new(0, 28)
                },
                recompute_line_start: 28,
                recompute_offset_end: usize::MAX,
                copy_line_end:        CopyDelta::None
            }
        );
    }
    {
        let rope_delta = RopeDelta {
            els:      vec![
                DeltaElement::<RopeInfo>::Insert(Node::from_leaf(
                    "\r\n".to_string()
                )),
                DeltaElement::<RopeInfo>::Copy(0, 461),
            ],
            base_len: 461
        };
        let offset =
            resolve_delta_compute(&rope_delta).ok_or(anyhow!("rs is none"))?;
        assert_eq!(
            offset,
            OffsetDelta {
                copy_start:   Interval::new(0, 0),
                internal_len: 2,
                copy_end:     Interval::new(0, 461)
            }
        );
        let rs = resolve_line_delta(lines.buffer().text(), offset)?;
        assert_eq!(
            rs,
            OriginLinesDelta {
                copy_line_start:      CopyDelta::None,
                recompute_line_start: 0,
                recompute_offset_end: 15,
                copy_line_end:        CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Offset::Add(2),
                    line_offset: Default::default(),
                    copy_line: Interval::new(1, 29)
                }
            }
        );
    }
    {
        let rope_delta = RopeDelta {
            els:      vec![
                DeltaElement::<RopeInfo>::Copy(0, 117),
                DeltaElement::<RopeInfo>::Insert(Node::from_leaf(
                    "\r\n    ".to_string()
                )),
                DeltaElement::<RopeInfo>::Copy(117, 461),
            ],
            base_len: 461
        };
        let offset =
            resolve_delta_compute(&rope_delta).ok_or(anyhow!("rs is none"))?;
        assert_eq!(
            offset,
            OffsetDelta {
                copy_start:   Interval::new(0, 117),
                internal_len: 6,
                copy_end:     Interval::new(117, 461)
            }
        );
        let rs = resolve_line_delta(lines.buffer().text(), offset)?;
        assert_eq!(
            rs,
            OriginLinesDelta {
                copy_line_start:      CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Default::default(),
                    line_offset: Default::default(),
                    copy_line: Interval::new(0, 6)
                },
                recompute_line_start: 6,
                recompute_offset_end: 131,
                copy_line_end:        CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Offset::Add(6),
                    line_offset: Default::default(),
                    copy_line: Interval::new(7, 29)
                }
            }
        );
    }
    {
        let rope_delta = RopeDelta {
            els:      vec![
                DeltaElement::<RopeInfo>::Copy(0, 117),
                DeltaElement::<RopeInfo>::Insert(Node::from_leaf("m".to_string())),
                DeltaElement::<RopeInfo>::Copy(117, 461),
            ],
            base_len: 461
        };
        let offset =
            resolve_delta_compute(&rope_delta).ok_or(anyhow!("rs is none"))?;
        assert_eq!(
            offset,
            OffsetDelta {
                copy_start:   Interval::new(0, 117),
                internal_len: 1,
                copy_end:     Interval::new(117, 461)
            }
        );
        let rs = resolve_line_delta(lines.buffer().text(), offset)?;
        assert_eq!(
            rs,
            OriginLinesDelta {
                copy_line_start:      CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Default::default(),
                    line_offset: Default::default(),
                    copy_line: Interval::new(0, 6)
                },
                recompute_line_start: 6,
                recompute_offset_end: 126,
                copy_line_end:        CopyDelta::Copy {
                    recompute_first_or_last_line: false,
                    offset: Offset::Add(1),
                    line_offset: Default::default(),
                    copy_line: Interval::new(7, 29)
                }
            }
        );
    }
    Ok(())
}
