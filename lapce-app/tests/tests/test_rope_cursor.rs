use lapce_xi_rope::{Cursor, Rope};
use log::debug;

use crate::tests::lines_util::*;

#[test]
fn peek_next_codepoint_panics_on_invalid_offset() {
    let rope = Rope::from("a導b"); // '導' 占 3 字节
    let mut cursor = Cursor::new(&rope, 3); // 3 是 '導' 字符中间，不是合法边界
    let _ = cursor.peek_next_codepoint(); // ⚠️ panic here
}

#[test]
fn chunk_iter_panics_on_invalid_start() {
    let rope = Rope::from("a導b"); // '導' 是 3 字节字符，位置 1~3
    let start = 2; // 非法字符中间位置
    let end = rope.len();
    let mut chunks = rope.iter_chunks(start..end);
    let _ = chunks.next(); // ⚠️ panic here
}
