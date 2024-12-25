#![allow(unused_imports)]
use crate::window_tab::WindowTabData;
use floem::reactive::SignalGet;
use itertools::Itertools;
use log::{debug, error, info, warn};

pub fn log(window: &WindowTabData) {
    print_screen_lines(window);
}

pub fn print_screen_lines(window: &WindowTabData) {
    for (_, editor) in &window.main_split.editors.0.get_untracked() {
        if let Some(path) = editor.doc().content.get_untracked().path() {
            warn!("{:?} {:?}", path, editor.editor.cursor.get_untracked());
            editor.doc().lines.with_untracked(|x| x.log());
            warn!("");
        }
        // if editor
        //     .doc()
        //     .name
        //     .as_ref()
        //     .is_some_and(|x| x == "PaletteData")
        // {
        //     editor.doc().lines.with_untracked(|x| x.log());
        //     warn!("");
        // }
    }
}
