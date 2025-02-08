use crate::lines::buffer::diff::DiffLines;

#[derive(Clone)]
pub struct DiffInfo {
    pub is_right: bool,
    pub changes:  Vec<DiffLines>
}
