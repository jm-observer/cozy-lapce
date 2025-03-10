#![allow(unused_imports)]
use floem::reactive::SignalGet;
use itertools::Itertools;
use lapce_core::doc::DocContent;
use log::{debug, error, info, warn};

use crate::window_workspace::WindowWorkspaceData;

pub fn log(window: &WindowWorkspaceData) {
    print_screen_lines(window);
}

pub fn print_screen_lines(window: &WindowWorkspaceData) {
    let editors = window.main_split.editors.0.get_untracked();
    info!("{} editors", editors.len());

    for (_, editor) in &editors {
        let content = editor.doc().content.get_untracked();
        match &content {
            DocContent::File { .. } | DocContent::History(_) => {
                warn!("{:?} {:?}", content, editor.editor.cursor.get_untracked());
                // editor.doc().lines.with_untracked(|x| x.log());
                warn!("");
                let screen_lines = editor.editor.screen_lines.get_untracked();
                screen_lines.log();
            },
            _ => {}
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
