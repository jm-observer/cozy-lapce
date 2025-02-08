use std::cmp::Ordering;

#[derive(Copy, Clone, Debug)]
pub enum Position {
    Region { start: usize, end: usize },
    Caret(usize),
    None
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub dragging: bool,
    pub position: Position
}

impl Cursor {
    pub fn offset(&self) -> Option<usize> {
        Some(match self.position {
            Position::Region { end, .. } => end,
            Position::Caret(offset) => offset,
            Position::None => return None
        })
    }

    pub fn start(&self) -> Option<usize> {
        Some(match self.position {
            Position::Region { start, .. } => start,
            Position::Caret(offset) => offset,
            Position::None => return None
        })
    }

    pub fn region(&self) -> Option<(usize, usize)> {
        if let Position::Region { start, end } = self.position {
            match start.cmp(&end) {
                Ordering::Less => Some((start, end)),
                Ordering::Equal => None,
                Ordering::Greater => Some((end, start))
            }
        } else {
            None
        }
    }
}
