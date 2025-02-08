use anyhow::Result;
use lapce_xi_rope::{
    Rope, RopeDelta, RopeInfo,
    delta::InsertDelta,
    multiset::{CountMatcher, Subset}
};
use log::error;
use tree_sitter::Point;

use crate::lines::buffer::{
    InsertsValueIter,
    rope_text::{RopeText, RopeTextRef}
};

#[derive(Clone)]
pub struct SyntaxEdit(pub(crate) Vec<tree_sitter::InputEdit>);

impl SyntaxEdit {
    pub fn new(edits: Vec<tree_sitter::InputEdit>) -> Self {
        Self(edits)
    }

    pub fn from_delta(text: &Rope, delta: RopeDelta) -> Result<SyntaxEdit> {
        let (ins_delta, deletes) = delta.factor();

        Self::from_factored_delta(text, &ins_delta, &deletes)
    }

    pub fn from_factored_delta(
        text: &Rope,
        ins_delta: &InsertDelta<RopeInfo>,
        deletes: &Subset
    ) -> Result<SyntaxEdit> {
        let deletes = deletes.transform_expand(&ins_delta.inserted_subset());

        let mut edits = Vec::new();

        let mut insert_edits: Vec<tree_sitter::InputEdit> = Vec::new();
        for insert in InsertsValueIter::new(ins_delta) {
            let start = insert.old_offset;
            let inserted = insert.node;
            insert_edits.push(create_insert_edit(text, start, inserted)?);
        }
        insert_edits.reverse();
        edits.append(&mut insert_edits);

        let text = ins_delta.apply(text);
        let mut delete_edits: Vec<tree_sitter::InputEdit> = deletes
            .range_iter(CountMatcher::NonZero)
            .filter_map(|(start, end)| match create_delete_edit(&text, start, end) {
                Ok(rs) => Some(rs),
                Err(err) => {
                    error!("{err:?}");
                    None
                }
            })
            .collect();
        delete_edits.reverse();
        edits.append(&mut delete_edits);

        Ok(SyntaxEdit::new(edits))
    }
}

fn point_at_offset(text: &Rope, offset: usize) -> Result<Point> {
    let text = RopeTextRef::new(text);
    let line = text.line_of_offset(offset);
    let col = text.offset_of_line(line + 1)?.saturating_sub(offset);
    Ok(Point::new(line, col))
}

fn traverse(point: Point, text: &str) -> Point {
    let Point {
        mut row,
        mut column
    } = point;

    for ch in text.chars() {
        if ch == '\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Point { row, column }
}

pub fn create_insert_edit(
    old_text: &Rope,
    start: usize,
    inserted: &Rope
) -> Result<tree_sitter::InputEdit> {
    let start_position = point_at_offset(old_text, start)?;
    Ok(tree_sitter::InputEdit {
        start_byte: start,
        old_end_byte: start,
        new_end_byte: start + inserted.len(),
        start_position,
        old_end_position: start_position,
        new_end_position: traverse(
            start_position,
            &inserted.slice_to_cow(0..inserted.len())
        )
    })
}

pub fn create_delete_edit(
    old_text: &Rope,
    start: usize,
    end: usize
) -> Result<tree_sitter::InputEdit> {
    let start_position = point_at_offset(old_text, start)?;
    let end_position = point_at_offset(old_text, end)?;
    Ok(tree_sitter::InputEdit {
        start_byte: start,
        // The old end byte position was at the end
        old_end_byte: end,
        // but since we're deleting everything up to it, it gets
        // 'moved' to where we start
        new_end_byte: start,

        start_position,
        old_end_position: end_position,
        new_end_position: start_position
    })
}
