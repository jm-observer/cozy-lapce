pub mod about;
pub mod alert;
pub mod app;
pub mod code_action;
pub mod code_lens;
pub mod command;
mod common;
pub mod completion;
pub mod config;
pub mod db;
pub mod debug;
pub mod doc;
pub mod editor;
pub mod editor_tab;
pub mod file_explorer;
pub mod find;
pub mod focus_text;
pub mod global_search;
pub mod history;
pub mod hover;
pub mod id;
pub mod inline_completion;
pub mod keymap;
pub mod keypress;
pub mod listener;
mod local_task;
mod log;
pub mod lsp;
pub mod main_split;
pub mod markdown;
pub mod palette;
pub mod panel;
pub mod plugin;
pub mod proxy;
pub mod rename;
pub mod settings;
pub mod snippet;
pub mod source_control;
pub mod status;
pub mod terminal;
pub mod text_area;
pub mod title;
pub mod update;
pub mod wave;
pub mod web_link;
pub mod window;
pub mod window_workspace;

extern crate core;
#[cfg(windows)]
extern crate windows_sys as windows;

use floem::{View, prelude::Svg, reactive::create_effect};

pub fn svg(svg_str: impl Fn() -> String + 'static) -> Svg {
    let content = svg_str();
    let svg = floem::views::svg(content);
    let id = svg.id();
    create_effect(move |_| {
        let new_svg_str = svg_str();
        id.update_state(new_svg_str);
    });
    svg
}
