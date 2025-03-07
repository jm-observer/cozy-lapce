use std::iter::Peekable;
use std::ops::Range;
use std::slice::Iter;
use std::vec::IntoIter;
use serde::{Deserialize, Serialize};
use crate::lines::buffer::Buffer;
use crate::lines::buffer::diff::DiffLines;
use crate::lines::buffer::rope_text::RopeText;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffInfo {
    pub is_right: bool,
    pub changes:  Vec<DiffLines>
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DiffResult {
    /// 对方新增/已方删除
    Empty {
        lines: Range<usize>,
    },
    /// 双方修改
    Changed {
        lines: Range<usize>,
    }
}

impl DiffResult {
    pub fn line(&self) -> &Range<usize> {
        match self {
            DiffResult::Empty { lines: line } => {line}
            DiffResult::Changed { lines: line } => {line}
        }
    }

    pub fn consume_line(&self, line: &usize) -> bool {
        match self {
            DiffResult::Empty { lines } => {lines.contains(line)}
            DiffResult::Changed { .. } => {false}
        }
    }
}

impl DiffInfo {
    pub fn left_changes(&self,) -> Vec<DiffResult> {
        log::info!("{}", serde_json::to_string(&self.changes).unwrap());
        let mut changes = self.changes.iter().peekable();
        let mut next_left_change_line: Option<Range<usize>> = None;
        let mut diff_tys = vec![];
        loop {
            if let Some(change) = changes.next() {
                match change {
                    DiffLines::Left(diff) => {
                        next_left_change_line = Some(diff.clone());
                        if let Some(DiffLines::Right(_)) = changes.peek() {
                            // edit
                            changes.next();
                        }
                        diff_tys.push(DiffResult::Changed { lines: diff.clone() });
                    }
                    DiffLines::Both(diff) => {
                        next_left_change_line = Some(diff.left.clone());
                    }
                    DiffLines::Right(diff) => {
                        diff_tys.push(match &next_left_change_line {
                            None => {
                                DiffResult::Empty { lines: 0..diff.len() }
                            }
                            Some(lines) => {
                                DiffResult::Empty { lines: lines.end..lines.end+ diff.len() }
                            }
                        });
                    }
                }
            } else {
                break;
            }
        }
        // log::info!("{}", serde_json::to_string(&diff_tys).unwrap());

        diff_tys
    }

    pub fn right_changes(&self,) -> Vec<DiffResult> {
        log::info!("{}", serde_json::to_string(&self.changes).unwrap());
        let mut changes = self.changes.iter().peekable();
        let mut next_right_change_line: Option<Range<usize>> = None;
        let mut diff_tys = vec![];
        loop {
            if let Some(change) = changes.next() {
                match change {
                    DiffLines::Left(left_diff) => {
                        if let Some(DiffLines::Right(diff)) = changes.peek() {
                            // edit
                            changes.next();
                            diff_tys.push(DiffResult::Changed { lines: diff.clone() });
                            next_right_change_line = Some(diff.clone());
                        } else {
                            diff_tys.push(match &next_right_change_line {
                                None => {
                                    DiffResult::Empty { lines: 0..left_diff.len() }
                                }
                                Some(lines) => {
                                    DiffResult::Empty { lines: lines.end..lines.end+ left_diff.len() }
                                }
                            })
                        }
                    }
                    DiffLines::Both(diff) => {
                        next_right_change_line = Some(diff.right.clone());
                    }
                    DiffLines::Right(diff) => {
                        diff_tys.push(match &next_right_change_line {
                            None => {
                                DiffResult::Changed { lines: 0..diff.len() }
                            }
                            Some(lines) => {
                                DiffResult::Changed { lines: lines.end..lines.end+ diff.len() }
                            }
                        });
                    }
                }
            } else {
                break;
            }
        }
        // log::info!("{}", serde_json::to_string(&diff_tys).unwrap());

        diff_tys
    }
}



pub fn consume_line(changes: &mut Peekable<IntoIter<DiffResult>>, line: usize) -> bool {
    loop {
        if let Some(diff) = changes.peek() {
            if diff.consume_line(&line) {
                return true;
            } else if diff.line().end <= line{
                changes.next();
                continue;
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
}

pub fn consume_lines_until_enough(changes: &mut Peekable<IntoIter<DiffResult>>, end_index: usize) -> usize {
    let mut index= 0;
    let mut line = 0;
    loop {
        if index >= end_index {
            break;
        }
        if consume_line(changes, line) {
            index += 1;
            continue;
        }
        line += 1;
        index += 1;
    }
    line
}