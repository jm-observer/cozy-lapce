use anyhow::Result;
use lapce_xi_rope::{DeltaElement, Interval, Rope, RopeDelta};
use log::debug;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Offset {
    #[default]
    None,
    Add(usize),
    Minus(usize),
}

impl Offset {
    pub fn new(origin: usize, new: usize) -> Self {
        if origin > new {
            Self::minus(origin - new)
        } else {
            Self::add(new - origin)
        }
    }

    pub fn add(offset: usize) -> Self {
        if offset == 0 {
            Self::None
        } else {
            Self::Add(offset)
        }
    }

    pub fn minus(offset: usize) -> Self {
        if offset == 0 {
            Self::None
        } else {
            Self::Minus(offset)
        }
    }

    pub fn adjust(&self, num: &mut usize) {
        match self {
            Offset::None => {},
            Offset::Add(offset) => *num += *offset,
            Offset::Minus(offset) => *num -= offset,
        }
    }

    pub fn adjust_new(&self, num: usize) -> usize {
        match self {
            Offset::None => num,
            Offset::Add(offset) => num + *offset,
            Offset::Minus(offset) => num - offset,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OffsetDelta {
    pub copy_start:   Interval,
    pub internal_len: usize,
    pub copy_end:     Interval,
}

impl Default for OffsetDelta {
    fn default() -> Self {
        Self {
            copy_start:   Interval::new(0, 0),
            internal_len: 0,
            copy_end:     Interval::new(0, 0),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OriginLinesDelta {
    pub copy_line_start:      CopyDelta,
    pub recompute_line_start: usize,
    pub recompute_offset_end: usize,
    pub copy_line_end:        CopyDelta,
}

impl Default for OriginLinesDelta {
    fn default() -> Self {
        Self {
            copy_line_start:      Default::default(),
            recompute_line_start: 0,
            recompute_offset_end: usize::MAX,
            copy_line_end:        Default::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum CopyDelta {
    #[default]
    None,
    Copy {
        /// 首行如果不完整则需要重新计算
        recompute_first_or_last_line: bool,
        /// 相对的旧buffer的偏移
        offset: Offset,
        /// 相对的旧buffer的偏移
        line_offset: Offset,
        copy_line: Interval,
    },
}

// impl CopyStartDelta {
//     pub fn is_empty(&self) -> bool {
//         matches!(self, Self::None)
//     }
// }
// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
// pub struct CopyEndDelta {
//     pub offset: Offset,
//     pub line_offset: Offset,
//     pub recompute_last_line: bool,
//     pub copy_line: Interval,
// }
pub fn resolve_delta_rs(rope: &Rope, delta: &RopeDelta) -> Result<OriginLinesDelta> {
    let delta_compute = resolve_delta_compute(delta).unwrap();
    debug!("{delta_compute:?}");
    resolve_line_delta(rope, delta_compute)
}

pub fn resolve_line_delta(
    rope: &Rope,
    offset_delta_compute: OffsetDelta,
) -> Result<OriginLinesDelta> {
    let mut copy_line_start = CopyDelta::default();
    let mut offset_end = 0;
    let mut line_start = 0;
    if !offset_delta_compute.copy_start.is_empty() {
        let copy_line_start_info = resolve_line_complete_by_start_offset(
            rope,
            offset_delta_compute.copy_start.start,
        )?;
        let copy_line_end_info = resolve_line_complete_by_end_offset(
            rope,
            offset_delta_compute.copy_start.end,
        )?;
        offset_end += offset_delta_compute.copy_start.size();
        if copy_line_end_info.0 > copy_line_start_info.0 {
            let recompute_first_line = copy_line_start_info.2;
            let copy_line =
                Interval::new(copy_line_start_info.0, copy_line_end_info.0);

            if recompute_first_line {
                line_start += 1;
            }
            let line_offset = Offset::new(copy_line_start_info.0, line_start);
            copy_line_start = CopyDelta::Copy {
                recompute_first_or_last_line: recompute_first_line,
                offset: Offset::minus(offset_delta_compute.copy_start.start),
                line_offset,
                copy_line,
            };
            line_start += copy_line_end_info.0 - copy_line_start_info.0;
        }
    }
    offset_end += offset_delta_compute.internal_len;
    let mut copy_line_end = CopyDelta::default();
    if !offset_delta_compute.copy_end.is_empty() {
        // let copy_line_start_info = resolve_line_complete_by_start_offset(rope,
        // offset_delta_compute.copy_end.start)?;
        let (line, offset_of_line) = {
            // 为什么line+1，因为无法判断该行是否被影响（也许是在行首加字符），
            // 因此索性不要这行
            let line = rope.line_of_offset(offset_delta_compute.copy_end.start);
            let line_offset = rope.offset_of_line(line + 1)?;
            (line + 1, line_offset)
        };
        let copy_line_end_info = resolve_line_complete_by_end_offset(
            rope,
            offset_delta_compute.copy_end.end,
        )?;
        if copy_line_end_info.0 > line {
            // offset_end += copy_line_start_info.1 -
            // offset_delta_compute.copy_end.start;
            let recompute_last_line = copy_line_end_info.2;
            let copy_line = if copy_line_end_info.2 {
                Interval::new(line, copy_line_end_info.0)
            } else {
                Interval::new(line, copy_line_end_info.0 + 1)
            };
            offset_end += offset_of_line - offset_delta_compute.copy_end.start;
            copy_line_end = CopyDelta::Copy {
                recompute_first_or_last_line: recompute_last_line,
                offset: Offset::new(offset_of_line, offset_end),
                copy_line,
                line_offset: Default::default(),
            };
        } else {
            offset_end = usize::MAX;
        }
    } else {
        offset_end = usize::MAX;
    }
    Ok(OriginLinesDelta {
        copy_line_start,
        recompute_line_start: line_start,
        recompute_offset_end: offset_end,
        copy_line_end,
    })
}

/// return (line, offset_line, recompute)
fn resolve_line_complete_by_start_offset(
    rope: &Rope,
    offset: usize,
) -> Result<(usize, usize, bool)> {
    let mut line = rope.line_of_offset(offset);
    let mut line_offset = rope.offset_of_line(line)?;
    let recompute = offset != line_offset;
    if recompute {
        line += 1;
        line_offset = rope.offset_of_line(line)?;
    }
    Ok((line, line_offset, recompute))
}

/// return (line, offset_line, recompute)
fn resolve_line_complete_by_end_offset(
    rope: &Rope,
    offset: usize,
) -> Result<(usize, usize, bool)> {
    let line = rope.line_of_offset(offset);
    let offset_line = rope.offset_of_line(line)?;
    let recompute = offset != offset_line;
    Ok((line, offset_line, recompute))
}

pub fn resolve_delta_compute(delta: &RopeDelta) -> Option<OffsetDelta> {
    let mut rs = OffsetDelta::default();
    let len = delta.els.len();
    debug!("{:?}", delta);
    match len {
        0 => {},
        1 => {
            let first = delta.els.first()?;
            match first {
                DeltaElement::Copy(start, end) => {
                    rs.copy_start = Interval::new(*start, *end);
                },
                DeltaElement::Insert(val) => {
                    rs.internal_len = val.len();
                },
            }
        },
        _ => {
            let (first, last) = (delta.els.first()?, delta.els.last()?);
            match (first, last) {
                (
                    DeltaElement::Copy(start, end),
                    DeltaElement::Copy(start_end, end_end),
                ) => {
                    rs.copy_start = Interval::new(*start, *end);
                    rs.copy_end = Interval::new(*start_end, *end_end);
                },
                (DeltaElement::Copy(start, end), DeltaElement::Insert(val_end)) => {
                    rs.copy_start = Interval::new(*start, *end);
                    rs.internal_len = val_end.len();
                },
                (
                    DeltaElement::Insert(val),
                    DeltaElement::Copy(start_end, end_end),
                ) => {
                    rs.internal_len = val.len();
                    rs.copy_end = Interval::new(*start_end, *end_end);
                },
                (DeltaElement::Insert(val), DeltaElement::Insert(val_end)) => {
                    rs.internal_len = val.len() + val_end.len();
                },
            }
            if len > 2 {
                let iter = delta.els[1..len - 1].iter();
                for delta in iter {
                    match delta {
                        DeltaElement::Copy(start, end) => {
                            rs.internal_len += *end - *start;
                        },
                        DeltaElement::Insert(val) => {
                            rs.internal_len += val.len();
                        },
                    }
                }
            }
        },
    }
    Some(rs)
}
