use floem::{
    IntoView, View,
    peniko::{Brush, Color},
    prelude::{palette, text},
    prop,
    style::{CursorColor, StylePropValue, TextColor},
    style_class,
};
use serde::{Deserialize, Serialize};

use crate::lines::{
    indent::IndentStyle,
    text::{RenderWhitespace, WrapMethod},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewLineStyle {
    pub origin_line: usize,
    /// 所在行的起始位置，废弃？？
    pub origin_line_offset_start: usize,
    pub len: usize,
    /// 在整个buffer的起始位置
    pub start_of_buffer: usize,
    pub end_of_buffer: usize,
    pub fg_color: Color, /* pub folded_line_offset_start: usize,
                          * pub folded_line_offset_end: usize /* pub fg_color:
                          *                                    * Option<String>,
                          *                                      */ */
}

// impl NewLineStyle {
//     // pub fn adjust(&mut self, offset: Offset, line_offset: Offset) {
//     //     offset.adjust(&mut self.start_of_buffer);
//     //     offset.adjust(&mut self.end_of_buffer);
//     //     line_offset.adjust(&mut self.origin_line);
//     // }
// }

prop!(pub WrapProp: WrapMethod {} = WrapMethod::EditorWidth);
impl StylePropValue for WrapMethod {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(text(self).into_any())
    }
}

prop!(pub CursorSurroundingLines: usize {} = 1);
prop!(pub ScrollBeyondLastLine: bool {} = false);
prop!(pub ShowIndentGuide: bool {} = false);
prop!(pub Modal: bool {} = false);
prop!(pub ModalRelativeLine: bool {} = false);
prop!(pub SmartTab: bool {} = false);
prop!(pub PhantomColor: Color {} = palette::css::DIM_GRAY);
prop!(pub PlaceholderColor: Color {} = palette::css::DIM_GRAY);
prop!(pub PreeditUnderlineColor: Color {} = Color::WHITE);
prop!(pub RenderWhitespaceProp: RenderWhitespace {} = RenderWhitespace::None);

impl StylePropValue for RenderWhitespace {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(text(self).into_any())
    }
}
prop!(pub IndentStyleProp: IndentStyle {} = IndentStyle::Spaces(4));
impl StylePropValue for IndentStyle {
    fn debug_view(&self) -> Option<Box<dyn View>> {
        Some(text(self).into_any())
    }
}

prop!(pub DropdownShadow: Option<Color> {} = None);
prop!(pub Foreground: Color { inherited } = Color::from_rgb8(0x38, 0x3A, 0x42));
prop!(pub Focus: Option<Color> {} = None);
prop!(pub SelectionColor: Color {} = Color::BLACK.multiply_alpha(0.5));
prop!(pub DocumentHighlightColor: Color {} = Color::from_rgb8(60, 116, 136));
prop!(pub CurrentLineColor: Option<Color> {  } = None);
prop!(pub Link: Option<Color> {} = None);
prop!(pub VisibleWhitespaceColor: Color {} = Color::TRANSPARENT);
prop!(pub IndentGuideColor: Color {} = Color::TRANSPARENT);
prop!(pub StickyHeaderBackground: Option<Color> {} = None);

floem::prop_extractor! {
    pub EditorStyle {
        pub text_color: TextColor,
        pub phantom_color: PhantomColor,
        pub placeholder_color: PlaceholderColor,
        pub preedit_underline_color: PreeditUnderlineColor,
        pub show_indent_guide: ShowIndentGuide,
        pub modal: Modal,
        // Whether line numbers are relative in modal mode
        pub modal_relative_line: ModalRelativeLine,
        // Whether to insert the indent that is detected for the file when a tab character
        // is inputted.
        pub smart_tab: SmartTab,
        pub wrap_method: WrapProp,
        pub cursor_surrounding_lines: CursorSurroundingLines,
        pub render_whitespace: RenderWhitespaceProp,
        pub indent_style: IndentStyleProp,
        pub caret: CursorColor,
        pub selection: SelectionColor,
        pub document_highlight: DocumentHighlightColor,
        pub current_line: CurrentLineColor,
        pub visible_whitespace: VisibleWhitespaceColor,
        pub indent_guide: IndentGuideColor,
        pub scroll_beyond_last_line: ScrollBeyondLastLine,
    }
}
impl EditorStyle {
    pub fn ed_text_color(&self) -> Color {
        self.text_color().unwrap_or(Color::BLACK)
    }
}
impl EditorStyle {
    pub fn ed_caret(&self) -> Brush {
        self.caret()
    }
}

style_class!(pub EditorViewClass);
