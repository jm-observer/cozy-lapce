use core::slice;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use floem::peniko::Color;
use floem::prelude::{palette, SignalWith};
use floem::reactive::{batch, ReadSignal, Scope};
use floem::text::FamilyOwned;
use log::error;
use lsp_types::{CompletionItemKind, SymbolKind};
use parking_lot::RwLock;
use doc::lines::signal::SignalManager;
use doc::lines::text::RenderWhitespace;
use lapce_core::icon::LapceIcons;
use crate::config::{LapceConfig, DEFAULT_ICON_THEME_ICON_CONFIG};
use crate::config::color::LapceColor;
use crate::config::editor::{ClickMode, EditorConfig, WrapStyle};
use crate::config::icon_theme::IconThemeConfig;
use crate::config::svg::SvgStore;
use crate::config::ui::{TabCloseButton, TabSeparatorHeight, UIConfig};

#[derive(Debug, Clone, Default)]
pub struct ThemeColorSignal {
    pub syntax: HashMap<String, SignalManager<Color>>,
    pub ui:     HashMap<String, SignalManager<Color>>
}


impl ThemeColorSignal {
    pub fn init(cx: Scope, config: &LapceConfig) -> Self {
        let syntax = config.color.syntax.iter().map(|x| (x.0.clone(), SignalManager::new(cx, x.1.clone()))).collect();
        let ui = config.color.ui.iter().map(|x| (x.0.clone(), SignalManager::new(cx, x.1.clone()))).collect();
        Self {
            syntax, ui
        }
    }
    pub fn update(&mut self, config: &LapceConfig) {
        config.color.syntax.iter().for_each(|(key, val)| {
            let Some(signal) = self.syntax.get_mut(key) else {
                return;
            };
            signal.update_and_trigger_if_not_equal(val.clone());
        });

        config.color.ui.iter().for_each(|(key, val)| {
            let Some(signal) = self.ui.get_mut(key) else {
                return;
            };
            signal.update_and_trigger_if_not_equal(val.clone());
        });
    }
}
#[derive(Clone)]

pub struct EditorConfigSignal {
    pub font_family: SignalManager<(Vec<FamilyOwned>, String)>,
    pub font_size: SignalManager<usize>,
    pub code_glance_font_size: SignalManager<usize>,
    pub line_height: SignalManager<usize>,
    pub smart_tab: SignalManager<bool>,
    pub tab_width: SignalManager<usize>,
    pub show_tab: SignalManager<bool>,
    pub show_bread_crumbs: SignalManager<bool>,
    pub scroll_beyond_last_line: SignalManager<bool>,
    pub cursor_surrounding_lines: SignalManager<usize>,
    pub wrap_style: SignalManager<WrapStyle>,
    pub wrap_width: SignalManager<usize>,
    pub sticky_header: SignalManager<bool>,
    pub completion_width: SignalManager<usize>,
    pub completion_show_documentation: SignalManager<bool>,
    pub completion_item_show_detail: SignalManager<bool>,
    pub show_signature: SignalManager<bool>,
    pub signature_label_code_block: SignalManager<bool>,
    pub auto_closing_matching_pairs: SignalManager<bool>,
    pub auto_surround: SignalManager<bool>,
    pub hover_delay: SignalManager<u64>,
    pub modal_mode_relative_line_numbers: SignalManager<bool>,
    pub format_on_save: SignalManager<bool>,
    pub normalize_line_endings: SignalManager<bool>,
    pub highlight_matching_brackets: SignalManager<bool>,
    pub highlight_scope_lines: SignalManager<bool>,
    pub enable_inlay_hints: SignalManager<bool>,
    pub inlay_hint_font_family: SignalManager<String>,
    pub inlay_hint_font_size: SignalManager<usize>,
    pub enable_error_lens: SignalManager<bool>,
    pub only_render_error_styling: SignalManager<bool>,
    pub error_lens_end_of_line: SignalManager<bool>,
    pub error_lens_multiline: SignalManager<bool>,
    pub error_lens_font_family: SignalManager<String>,
    pub error_lens_font_size: SignalManager<usize>,
    pub enable_completion_lens: SignalManager<bool>,
    pub enable_inline_completion: SignalManager<bool>,
    pub completion_lens_font_family: SignalManager<String>,
    pub completion_lens_font_size: SignalManager<usize>,
    pub blink_interval: SignalManager<u64>,
    pub multicursor_case_sensitive: SignalManager<bool>,
    pub multicursor_whole_words: SignalManager<bool>,
    pub render_whitespace: SignalManager<RenderWhitespace>,
    pub show_indent_guide: SignalManager<bool>,
    pub autosave_interval: SignalManager<u64>,
    pub format_on_autosave: SignalManager<bool>,
    pub atomic_soft_tabs: SignalManager<bool>,
    pub double_click: SignalManager<ClickMode>,
    pub move_focus_while_search: SignalManager<bool>,
    pub diff_context_lines: SignalManager<i32>,
    pub bracket_pair_colorization: SignalManager<bool>,
    pub bracket_colorization_limit: SignalManager<u64>,
    pub files_exclude: SignalManager<String>,
}

impl EditorConfigSignal {
    pub fn init(cx: Scope, config: &EditorConfig) -> Self {
        let font_family = SignalManager::new(cx, (FamilyOwned::parse_list(&config.font_family).collect(), config.font_family.clone()));
        let font_size = SignalManager::new(cx, config.font_size());
        let code_glance_font_size = SignalManager::new(cx, config.code_glance_font_size);
        let line_height = SignalManager::new(cx, config.line_height());
        let smart_tab = SignalManager::new(cx, config.smart_tab);
        let tab_width = SignalManager::new(cx, config.tab_width);
        let show_tab = SignalManager::new(cx, config.show_tab);
        let show_bread_crumbs = SignalManager::new(cx, config.show_bread_crumbs);
        let scroll_beyond_last_line = SignalManager::new(cx, config.scroll_beyond_last_line);
        let cursor_surrounding_lines = SignalManager::new(cx, config.cursor_surrounding_lines);
        let wrap_style = SignalManager::new(cx, config.wrap_style);
        let wrap_width = SignalManager::new(cx, config.wrap_width);
        let sticky_header = SignalManager::new(cx, config.sticky_header);
        let completion_width = SignalManager::new(cx, config.completion_width);
        let completion_show_documentation = SignalManager::new(cx, config.completion_show_documentation);
        let completion_item_show_detail = SignalManager::new(cx, config.completion_item_show_detail);
        let show_signature = SignalManager::new(cx, config.show_signature);
        let signature_label_code_block = SignalManager::new(cx, config.signature_label_code_block);
        let auto_closing_matching_pairs = SignalManager::new(cx, config.auto_closing_matching_pairs);
        let auto_surround = SignalManager::new(cx, config.auto_surround);
        let hover_delay = SignalManager::new(cx, config.hover_delay);
        let modal_mode_relative_line_numbers = SignalManager::new(cx, config.modal_mode_relative_line_numbers);
        let format_on_save = SignalManager::new(cx, config.format_on_save);
        let normalize_line_endings = SignalManager::new(cx, config.normalize_line_endings);
        let highlight_matching_brackets = SignalManager::new(cx, config.highlight_matching_brackets);
        let highlight_scope_lines = SignalManager::new(cx, config.highlight_scope_lines);
        let enable_inlay_hints = SignalManager::new(cx, config.enable_inlay_hints);
        let inlay_hint_font_family = SignalManager::new(cx, config.inlay_hint_font_family.clone());
        let inlay_hint_font_size = SignalManager::new(cx, config.inlay_hint_font_size);
        let enable_error_lens = SignalManager::new(cx, config.enable_error_lens);
        let only_render_error_styling = SignalManager::new(cx, config.only_render_error_styling);
        let error_lens_end_of_line = SignalManager::new(cx, config.error_lens_end_of_line);
        let error_lens_multiline = SignalManager::new(cx, config.error_lens_multiline);
        let error_lens_font_family = SignalManager::new(cx, config.error_lens_font_family.clone());
        let error_lens_font_size = SignalManager::new(cx, config.error_lens_font_size);
        let enable_completion_lens = SignalManager::new(cx, config.enable_completion_lens);
        let enable_inline_completion = SignalManager::new(cx, config.enable_inline_completion);
        let completion_lens_font_family = SignalManager::new(cx, config.completion_lens_font_family.clone());
        let completion_lens_font_size = SignalManager::new(cx, config.completion_lens_font_size);
        let blink_interval = SignalManager::new(cx, config.blink_interval());
        let multicursor_case_sensitive = SignalManager::new(cx, config.multicursor_case_sensitive);
        let multicursor_whole_words = SignalManager::new(cx, config.multicursor_whole_words);
        let render_whitespace = SignalManager::new(cx, config.render_whitespace);
        let show_indent_guide = SignalManager::new(cx, config.show_indent_guide);
        let autosave_interval = SignalManager::new(cx, config.autosave_interval);
        let format_on_autosave = SignalManager::new(cx, config.format_on_autosave);
        let atomic_soft_tabs = SignalManager::new(cx, config.atomic_soft_tabs);
        let double_click = SignalManager::new(cx, config.double_click);
        let move_focus_while_search = SignalManager::new(cx, config.move_focus_while_search);
        let diff_context_lines = SignalManager::new(cx, config.diff_context_lines);
        let bracket_pair_colorization = SignalManager::new(cx, config.bracket_pair_colorization);
        let bracket_colorization_limit = SignalManager::new(cx, config.bracket_colorization_limit);
        let files_exclude = SignalManager::new(cx, config.files_exclude.clone());

        Self {
            font_family,
            font_size,
            code_glance_font_size,
            line_height,
            smart_tab,
            tab_width,
            show_tab,
            show_bread_crumbs,
            scroll_beyond_last_line,
            cursor_surrounding_lines,
            wrap_style,
            wrap_width,
            sticky_header,
            completion_width,
            completion_show_documentation,
            completion_item_show_detail,
            show_signature,
            signature_label_code_block,
            auto_closing_matching_pairs,
            auto_surround,
            hover_delay,
            modal_mode_relative_line_numbers,
            format_on_save,
            normalize_line_endings,
            highlight_matching_brackets,
            highlight_scope_lines,
            enable_inlay_hints,
            inlay_hint_font_family,
            inlay_hint_font_size,
            enable_error_lens,
            only_render_error_styling,
            error_lens_end_of_line,
            error_lens_multiline,
            error_lens_font_family,
            error_lens_font_size,
            enable_completion_lens,
            enable_inline_completion,
            completion_lens_font_family,
            completion_lens_font_size,
            blink_interval,
            multicursor_case_sensitive,
            multicursor_whole_words,
            render_whitespace,
            show_indent_guide,
            autosave_interval,
            format_on_autosave,
            atomic_soft_tabs,
            double_click,
            move_focus_while_search,
            diff_context_lines,
            bracket_pair_colorization,
            bracket_colorization_limit,
            files_exclude,
        }
    }

    pub fn update(&mut self, config: &EditorConfig) {
        self.font_family.update_and_trigger_if_not_equal((FamilyOwned::parse_list(&config.font_family).collect(), config.font_family.clone()));
        self.font_size.update_and_trigger_if_not_equal(config.font_size());
        self.code_glance_font_size.update_and_trigger_if_not_equal(config.code_glance_font_size);
        self.line_height.update_and_trigger_if_not_equal(config.line_height());
        self.smart_tab.update_and_trigger_if_not_equal(config.smart_tab);
        self.tab_width.update_and_trigger_if_not_equal(config.tab_width);
        self.show_tab.update_and_trigger_if_not_equal(config.show_tab);
        self.show_bread_crumbs.update_and_trigger_if_not_equal(config.show_bread_crumbs);
        self.scroll_beyond_last_line.update_and_trigger_if_not_equal(config.scroll_beyond_last_line);
        self.cursor_surrounding_lines.update_and_trigger_if_not_equal(config.cursor_surrounding_lines);
        self.wrap_style.update_and_trigger_if_not_equal(config.wrap_style);
        self.wrap_width.update_and_trigger_if_not_equal(config.wrap_width);
        self.sticky_header.update_and_trigger_if_not_equal(config.sticky_header);
        self.completion_width.update_and_trigger_if_not_equal(config.completion_width);
        self.completion_show_documentation.update_and_trigger_if_not_equal(config.completion_show_documentation);
        self.completion_item_show_detail.update_and_trigger_if_not_equal(config.completion_item_show_detail);
        self.show_signature.update_and_trigger_if_not_equal(config.show_signature);
        self.signature_label_code_block.update_and_trigger_if_not_equal(config.signature_label_code_block);
        self.auto_closing_matching_pairs.update_and_trigger_if_not_equal(config.auto_closing_matching_pairs);
        self.auto_surround.update_and_trigger_if_not_equal(config.auto_surround);
        self.hover_delay.update_and_trigger_if_not_equal(config.hover_delay);
        self.modal_mode_relative_line_numbers.update_and_trigger_if_not_equal(config.modal_mode_relative_line_numbers);
        self.format_on_save.update_and_trigger_if_not_equal(config.format_on_save);
        self.normalize_line_endings.update_and_trigger_if_not_equal(config.normalize_line_endings);
        self.highlight_matching_brackets.update_and_trigger_if_not_equal(config.highlight_matching_brackets);
        self.highlight_scope_lines.update_and_trigger_if_not_equal(config.highlight_scope_lines);
        self.enable_inlay_hints.update_and_trigger_if_not_equal(config.enable_inlay_hints);
        self.inlay_hint_font_family.update_and_trigger_if_not_equal(config.inlay_hint_font_family.clone());
        self.inlay_hint_font_size.update_and_trigger_if_not_equal(config.inlay_hint_font_size);
        self.enable_error_lens.update_and_trigger_if_not_equal(config.enable_error_lens);
        self.only_render_error_styling.update_and_trigger_if_not_equal(config.only_render_error_styling);
        self.error_lens_end_of_line.update_and_trigger_if_not_equal(config.error_lens_end_of_line);
        self.error_lens_multiline.update_and_trigger_if_not_equal(config.error_lens_multiline);
        self.error_lens_font_family.update_and_trigger_if_not_equal(config.error_lens_font_family.clone());
        self.error_lens_font_size.update_and_trigger_if_not_equal(config.error_lens_font_size);
        self.enable_completion_lens.update_and_trigger_if_not_equal(config.enable_completion_lens);
        self.enable_inline_completion.update_and_trigger_if_not_equal(config.enable_inline_completion);
        self.completion_lens_font_family.update_and_trigger_if_not_equal(config.completion_lens_font_family.clone());
        self.completion_lens_font_size.update_and_trigger_if_not_equal(config.completion_lens_font_size);
        self.blink_interval.update_and_trigger_if_not_equal(config.blink_interval());
        self.multicursor_case_sensitive.update_and_trigger_if_not_equal(config.multicursor_case_sensitive);
        self.multicursor_whole_words.update_and_trigger_if_not_equal(config.multicursor_whole_words);
        self.render_whitespace.update_and_trigger_if_not_equal(config.render_whitespace);
        self.show_indent_guide.update_and_trigger_if_not_equal(config.show_indent_guide);
        self.autosave_interval.update_and_trigger_if_not_equal(config.autosave_interval);
        self.format_on_autosave.update_and_trigger_if_not_equal(config.format_on_autosave);
        self.atomic_soft_tabs.update_and_trigger_if_not_equal(config.atomic_soft_tabs);
        self.double_click.update_and_trigger_if_not_equal(config.double_click);
        self.move_focus_while_search.update_and_trigger_if_not_equal(config.move_focus_while_search);
        self.diff_context_lines.update_and_trigger_if_not_equal(config.diff_context_lines);
        self.bracket_pair_colorization.update_and_trigger_if_not_equal(config.bracket_pair_colorization);
        self.bracket_colorization_limit.update_and_trigger_if_not_equal(config.bracket_colorization_limit);
        self.files_exclude.update_and_trigger_if_not_equal(config.files_exclude.clone());
    }
}



#[derive(Clone)]
pub struct UiConfigSignal {
    pub scale: SignalManager<f64>,
    pub font_size: SignalManager<usize>,
    pub font_family: SignalManager<(Vec<FamilyOwned>, String)>,
    pub header_height: SignalManager<usize>,
    pub icon_size: SignalManager<usize>,
    pub status_height: SignalManager<usize>,
    pub palette_width: SignalManager<usize>,
    pub tab_close_button: SignalManager<TabCloseButton>,
    pub tab_separator_height: SignalManager<TabSeparatorHeight>,
    pub trim_search_results_whitespace: SignalManager<bool>,
    pub open_editors_visible: SignalManager<bool>,

}

impl UiConfigSignal {
    pub fn init(cx: Scope, config: &UIConfig) -> Self {
        let scale: SignalManager<f64>= SignalManager::new(cx,  config.scale());
        let font_size: SignalManager<usize>= SignalManager::new(cx,  config.font_size());
        let font_family = SignalManager::new(cx,  (config.font_family(), config.font_family.clone()));
        let header_height: SignalManager<usize>= SignalManager::new(cx,  config.header_height());
        let icon_size: SignalManager<usize>= SignalManager::new(cx,  config.icon_size());
        let status_height: SignalManager<usize>= SignalManager::new(cx,  config.status_height());
        let palette_width: SignalManager<usize>= SignalManager::new(cx,  config.palette_width());
        let tab_close_button = SignalManager::new(cx,  config.tab_close_button);
        let tab_separator_height = SignalManager::new(cx,  config.tab_separator_height);
        let trim_search_results_whitespace = SignalManager::new(cx,  config.trim_search_results_whitespace);
        let open_editors_visible = SignalManager::new(cx,  config.open_editors_visible);

        Self {
            scale,
            font_size,
            font_family,
            header_height,
            icon_size,
            status_height,
            palette_width,
            tab_close_button, tab_separator_height, trim_search_results_whitespace, open_editors_visible
        }
    }

    pub fn update(&mut self, config: &UIConfig) {
        self.scale.update_and_trigger_if_not_equal(config.scale());
        self.font_size.update_and_trigger_if_not_equal(config.font_size());
        self.font_family.update_and_trigger_if_not_equal((config.font_family(), config.font_family.clone()));
        self.header_height.update_and_trigger_if_not_equal(config.header_height());
        self.icon_size.update_and_trigger_if_not_equal(config.icon_size());
        self.status_height.update_and_trigger_if_not_equal(config.status_height());
        self.palette_width.update_and_trigger_if_not_equal(config.palette_width());
        self.tab_close_button.update_and_trigger_if_not_equal(config.tab_close_button);
        self.tab_separator_height.update_and_trigger_if_not_equal(config.tab_separator_height);
        self.trim_search_results_whitespace.update_and_trigger_if_not_equal(config.trim_search_results_whitespace);
        self.open_editors_visible.update_and_trigger_if_not_equal(config.open_editors_visible);

    }
}


#[derive(Clone)]
pub struct IconThemeConfigSignal {
    pub icon_theme: IconThemeConfig,
    pub svg_store:        Arc<RwLock<SvgStore>>,
    pub icon_active_color: Color,
}

impl PartialEq for IconThemeConfigSignal {
    fn eq(&self, other: &Self) -> bool {
        self.icon_theme == other.icon_theme && self.icon_active_color == other.icon_active_color
    }
}
impl Eq for IconThemeConfigSignal {
}

impl IconThemeConfigSignal {

    pub fn ui_svg(&self, icon: &'static str) -> String {
        let svg = self.icon_theme.ui.get(icon).and_then(|path| {
            let path = self.icon_theme.path.join(path);
            self.svg_store.write().get_svg_on_disk(&path)
        });

        svg.unwrap_or_else(|| {
            let name = DEFAULT_ICON_THEME_ICON_CONFIG.ui.get(icon).unwrap();
            self.svg_store.write().get_default_svg(name)
        })
    }

    pub fn files_svg(&self, paths: &[&Path]) -> (String, Option<Color>) {
        let svg = self
            .icon_theme
            .resolve_path_to_icon(paths)
            .and_then(|p| self.svg_store.write().get_svg_on_disk(&p));

        if let Some(svg) = svg {
            let color = if self.icon_theme.use_editor_color.unwrap_or(false) {
                Some(self.icon_active_color)
            } else {
                None
            };
            (svg, color)
        } else {
            (
                self.ui_svg(LapceIcons::FILE),
                Some(self.icon_active_color)
            )
        }
    }

    pub fn file_svg(&self, path: &Path) -> (String, Option<Color>) {
        self.files_svg(slice::from_ref(&path))
    }

    pub fn symbol_svg(&self, kind: &SymbolKind) -> Option<String> {
        let kind_str = match *kind {
            SymbolKind::ARRAY => LapceIcons::SYMBOL_KIND_ARRAY,
            SymbolKind::BOOLEAN => LapceIcons::SYMBOL_KIND_BOOLEAN,
            SymbolKind::CLASS => LapceIcons::SYMBOL_KIND_CLASS,
            SymbolKind::CONSTANT => LapceIcons::SYMBOL_KIND_CONSTANT,
            SymbolKind::ENUM_MEMBER => LapceIcons::SYMBOL_KIND_ENUM_MEMBER,
            SymbolKind::ENUM => LapceIcons::SYMBOL_KIND_ENUM,
            SymbolKind::EVENT => LapceIcons::SYMBOL_KIND_EVENT,
            SymbolKind::FIELD => LapceIcons::SYMBOL_KIND_FIELD,
            SymbolKind::FILE => LapceIcons::SYMBOL_KIND_FILE,
            SymbolKind::INTERFACE => LapceIcons::SYMBOL_KIND_INTERFACE,
            SymbolKind::KEY => LapceIcons::SYMBOL_KIND_KEY,
            SymbolKind::FUNCTION => LapceIcons::SYMBOL_KIND_FUNCTION,
            SymbolKind::METHOD => LapceIcons::SYMBOL_KIND_METHOD,
            SymbolKind::OBJECT => LapceIcons::SYMBOL_KIND_OBJECT,
            SymbolKind::NAMESPACE => LapceIcons::SYMBOL_KIND_NAMESPACE,
            SymbolKind::NUMBER => LapceIcons::SYMBOL_KIND_NUMBER,
            SymbolKind::OPERATOR => LapceIcons::SYMBOL_KIND_OPERATOR,
            SymbolKind::TYPE_PARAMETER => LapceIcons::SYMBOL_KIND_TYPE_PARAMETER,
            SymbolKind::PROPERTY => LapceIcons::SYMBOL_KIND_PROPERTY,
            SymbolKind::STRING => LapceIcons::SYMBOL_KIND_STRING,
            SymbolKind::STRUCT => LapceIcons::SYMBOL_KIND_STRUCT,
            SymbolKind::VARIABLE => LapceIcons::SYMBOL_KIND_VARIABLE,
            _ => return None
        };

        Some(self.ui_svg(kind_str))
    }
}


#[derive(Clone)]
pub struct LapceConfigSignal {
    pub cx: Scope,
    pub color: ThemeColorSignal,
    pub default_color: SignalManager<Color>,
    pub ui: UiConfigSignal,
    pub editor: EditorConfigSignal,
    pub icon_theme:             SignalManager<IconThemeConfigSignal>,
    pub svg_store:        Arc<RwLock<SvgStore>>,
}

impl LapceConfigSignal {

    pub fn init(cx: Scope, config: &LapceConfig) -> Self {
        let color = crate::config::signal::ThemeColorSignal::init(cx, config);
        let default_color = SignalManager::new(cx, palette::css::HOT_PINK);
        let ui = UiConfigSignal::init(cx, &config.ui);


        let editor = EditorConfigSignal::init(cx, &config.editor);
        let svg_store = Arc::new(RwLock::new(SvgStore::default()));
        let icon_active_color = config.color(LapceColor::LAPCE_ICON_ACTIVE);
        let icon_theme = crate::config::signal::IconThemeConfigSignal {
            icon_theme: config.icon_theme.clone(),
            svg_store: svg_store.clone(),
            icon_active_color,
        };
        let icon_theme = SignalManager::new(cx, icon_theme);
        Self {
            cx,
            color, default_color, ui, editor, svg_store, icon_theme
        }
    }

    pub fn update(&mut self, config: &LapceConfig) {
        let icon_active_color = config.color(LapceColor::LAPCE_ICON_ACTIVE);
        let icon_theme = IconThemeConfigSignal {
            icon_theme: config.icon_theme.clone(),
            svg_store: self.svg_store.clone(),
            icon_active_color,
        };

        batch(|| {
            self.color.update(config);
            self.ui.update(&config.ui);
            self.editor.update(&config.editor);
            self.icon_theme.update_and_trigger_if_not_equal(icon_theme);
        });
    }

    pub fn color(&self, name: &str) -> ReadSignal<Color> {
        match self.color.ui.get(name) {
            Some(c) => c.signal(),
            None => {
                error!("Failed to find key: {name}");
                self.default_color.signal()
            }
        }
    }
    pub fn color_val(&self, name: &str) -> Color {
        match self.color.ui.get(name) {
            Some(c) => c.val().clone(),
            None => {
                error!("Failed to find key: {name}");
                self.default_color.val().clone()
            }
        }
    }

    pub fn style_color(&self, name: &str) -> Option<ReadSignal<Color>> {
        self.color.syntax.get(name).map(|x| x.signal())
    }

    pub fn symbol_color(&self, kind: &SymbolKind) -> Option<ReadSignal<Color>> {
        let theme_str = match *kind {
            SymbolKind::METHOD => "method",
            SymbolKind::FUNCTION => "method",
            SymbolKind::ENUM => "enum",
            SymbolKind::ENUM_MEMBER => "enum-member",
            SymbolKind::CLASS => "class",
            SymbolKind::VARIABLE => "field",
            SymbolKind::STRUCT => "structure",
            SymbolKind::CONSTANT => "constant",
            SymbolKind::PROPERTY => "property",
            SymbolKind::FIELD => "field",
            SymbolKind::INTERFACE => "interface",
            SymbolKind::ARRAY => "",
            SymbolKind::BOOLEAN => "",
            SymbolKind::EVENT => "",
            SymbolKind::FILE => "",
            SymbolKind::KEY => "",
            SymbolKind::OBJECT => "",
            SymbolKind::NAMESPACE => "",
            SymbolKind::NUMBER => "number",
            SymbolKind::OPERATOR => "",
            SymbolKind::TYPE_PARAMETER => "",
            SymbolKind::STRING => "string",
            _ => return None
        };

        self.style_color(theme_str)
    }

    pub fn completion_color(
        &self,
        kind: Option<CompletionItemKind>
    ) -> Option<ReadSignal<Color>> {
        let kind = kind?;
        let theme_str = match kind {
            CompletionItemKind::METHOD => "method",
            CompletionItemKind::FUNCTION => "method",
            CompletionItemKind::ENUM => "enum",
            CompletionItemKind::ENUM_MEMBER => "enum-member",
            CompletionItemKind::CLASS => "class",
            CompletionItemKind::VARIABLE => "field",
            CompletionItemKind::STRUCT => "structure",
            CompletionItemKind::KEYWORD => "keyword",
            CompletionItemKind::CONSTANT => "constant",
            CompletionItemKind::PROPERTY => "property",
            CompletionItemKind::FIELD => "field",
            CompletionItemKind::INTERFACE => "interface",
            CompletionItemKind::SNIPPET => "snippet",
            CompletionItemKind::MODULE => "builtinType",
            _ => "string"
        };

        self.style_color(theme_str)
    }

    pub fn ui_svg(&self, key: &'static str) -> UiSvgSignal {
        UiSvgSignal {
            key,
            signal:self.icon_theme.signal(),
        }
    }

}

pub struct UiSvgSignal {
    key: &'static str,
    signal: ReadSignal<IconThemeConfigSignal>
}
impl UiSvgSignal {
    pub fn get(&self) -> String {
        self.signal.with(|x| x.ui_svg(self.key))
    }
}