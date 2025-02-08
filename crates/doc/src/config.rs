use std::collections::HashMap;

use floem::peniko::Color;
use lsp_types::DiagnosticSeverity;
use serde::{Deserialize, Serialize};

pub const SCALE_OR_SIZE_LIMIT: f64 = 5.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum WrapStyle {
    /// No wrapping
    None,
    /// Wrap at the editor width
    #[default]
    EditorWidth,
    // /// Wrap at the wrap-column
    // WrapColumn,
    /// Wrap at a specific width
    WrapWidth
}
impl WrapStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            WrapStyle::None => "none",
            WrapStyle::EditorWidth => "editor-width",
            // WrapStyle::WrapColumn => "wrap-column",
            WrapStyle::WrapWidth => "wrap-width"
        }
    }

    pub fn try_from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(WrapStyle::None),
            "editor-width" => Some(WrapStyle::EditorWidth),
            // "wrap-column" => Some(WrapStyle::WrapColumn),
            "wrap-width" => Some(WrapStyle::WrapWidth),
            _ => None
        }
    }
}

impl std::fmt::Display for WrapStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())?;

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EditorConfig {
    pub font_family:               String,
    pub font_size:                 usize,
    pub line_height:               usize,
    pub enable_inlay_hints:        bool,
    pub inlay_hint_font_size:      usize,
    pub enable_error_lens:         bool,
    pub error_lens_end_of_line:    bool,
    pub error_lens_multiline:      bool,
    pub error_lens_font_size:      usize,
    pub enable_completion_lens:    bool,
    pub enable_inline_completion:  bool,
    pub completion_lens_font_size: usize,
    pub only_render_error_styling: bool,

    pub auto_closing_matching_pairs: bool,
    pub auto_surround:               bool,

    pub diagnostic_error: Color,
    pub diagnostic_warn:  Color,
    pub inlay_hint_fg:    Color,
    pub inlay_hint_bg:    Color,

    pub error_lens_error_foreground:   Color,
    pub error_lens_warning_foreground: Color,
    pub error_lens_other_foreground:   Color,

    pub completion_lens_foreground: Color,

    pub editor_foreground: Color,

    pub syntax: HashMap<String, Color>
}

impl EditorConfig {
    pub fn inlay_hint_font_size(&self) -> usize {
        if self.inlay_hint_font_size < 5
            || self.inlay_hint_font_size > self.font_size
        {
            self.font_size
        } else {
            self.inlay_hint_font_size
        }
    }

    pub fn error_lens_font_size(&self) -> usize {
        if self.error_lens_font_size == 0 {
            self.inlay_hint_font_size()
        } else {
            self.error_lens_font_size
        }
    }

    pub fn completion_lens_font_size(&self) -> usize {
        if self.completion_lens_font_size == 0 {
            self.inlay_hint_font_size()
        } else {
            self.completion_lens_font_size
        }
    }

    // /// Returns the tab width if atomic soft tabs are enabled.
    // pub fn atomic_soft_tab_width(&self) -> Option<usize> {
    //     if self.atomic_soft_tabs {
    //         Some(self.tab_width)
    //     } else {
    //         None
    //     }
    // }

    // pub fn blink_interval(&self) -> u64 {
    //     if self.blink_interval == 0 {
    //         return 0;
    //     }
    //     self.blink_interval.max(200)
    // }

    pub fn color_of_diagnostic(
        &self,
        diagnostic_severity: DiagnosticSeverity
    ) -> Option<Color> {
        use DiagnosticSeverity;
        match diagnostic_severity {
            DiagnosticSeverity::ERROR => Some(self.diagnostic_error),
            DiagnosticSeverity::WARNING => Some(self.diagnostic_warn),
            _ => None
        }
    }

    pub fn color_of_error_lens(
        &self,
        diagnostic_severity: DiagnosticSeverity
    ) -> Color {
        match diagnostic_severity {
            DiagnosticSeverity::ERROR => self.error_lens_error_foreground,
            DiagnosticSeverity::WARNING => self.error_lens_warning_foreground,
            _ => self.error_lens_other_foreground
        }
    }

    pub fn syntax_style_color(&self, name: &str) -> Option<Color> {
        match name {
            "boolean" => self.syntax.get("constant").copied(),
            // "macro" => ?
            // "operator" => ?,
            _ => self.syntax.get(name).copied()
        }
    }
}
