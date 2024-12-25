#![allow(clippy::manual_clamp)]

pub mod directory;
pub mod lens;
pub mod meta;
pub mod rope_text_pos;
pub mod style;

pub mod encoding;

// This is primarily being re-exported to avoid changing every single usage
// in lapce-app. We should probably remove this at some point.
// pub use floem_editor_core::*;
