use std::collections::HashMap;

use doc::{language::LapceLanguage, syntax::Syntax};
use floem::{
    prelude::Color,
    text::{
        Attrs, AttrsList, FamilyOwned, LineHeightValue, Style, TextLayout, Weight
    }
};
use lapce_core::directory::Directory;
use lapce_xi_rope::Rope;
use log::warn;
use lsp_types::MarkedString;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag};
use smallvec::SmallVec;

#[derive(Clone)]
pub enum MarkdownContent {
    Text(TextLayout),
    Image { url: String, title: String },
    Separator
}

pub fn parse_markdown(
    text: &str,
    line_height: f64,
    directory: &Directory,
    font_family: &str,
    editor_fg: Color,
    style_colors: &HashMap<String, Color>,
    font_size: f32,
    markdown_blockquote: Color,
    editor_link: Color
) -> Vec<MarkdownContent> {
    let mut res = Vec::new();
    let mut current_text = String::new();
    let code_font_family: Vec<FamilyOwned> =
        FamilyOwned::parse_list(font_family).collect();

    let default_attrs = Attrs::new()
        .color(editor_fg)
        .font_size(font_size)
        .line_height(LineHeightValue::Normal(line_height as f32));
    let mut attr_list = AttrsList::new(default_attrs);

    let mut builder_dirty = false;

    let mut pos = 0;

    let mut tag_stack: SmallVec<[(usize, Tag); 4]> = SmallVec::new();

    let parser = Parser::new_ext(
        text,
        Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_HEADING_ATTRIBUTES
    );
    let mut last_text = CowStr::from("");
    // Whether we should add a newline on the next entry
    // This is used so that we don't emit newlines at the very end of the generation
    let mut add_newline = false;
    for event in parser {
        // Add the newline since we're going to be outputting more
        if add_newline {
            current_text.push('\n');
            builder_dirty = true;
            pos += 1;
            add_newline = false;
        }

        match event {
            Event::Start(tag) => {
                tag_stack.push((pos, tag));
            },
            Event::End(end_tag) => {
                if let Some((start_offset, tag)) = tag_stack.pop() {
                    if end_tag != tag.to_end() {
                        log::warn!("Mismatched markdown tag");
                        continue;
                    }

                    if let Some(attrs) = attribute_for_tag(
                        default_attrs,
                        &tag,
                        &code_font_family,
                        font_size,
                        markdown_blockquote,
                        editor_link
                    ) {
                        attr_list
                            .add_span(start_offset..pos.max(start_offset), attrs);
                    }

                    if should_add_newline_after_tag(&tag) {
                        add_newline = true;
                    }

                    match &tag {
                        Tag::CodeBlock(kind) => {
                            let language =
                                if let CodeBlockKind::Fenced(language) = kind {
                                    md_language_to_lapce_language(language)
                                } else {
                                    None
                                };

                            highlight_as_code(
                                &mut attr_list,
                                default_attrs.family(&code_font_family),
                                language,
                                &last_text,
                                start_offset,
                                directory,
                                style_colors
                            );
                            builder_dirty = true;
                        },
                        Tag::Image {
                            link_type: _,
                            dest_url: dest,
                            title,
                            id: _
                        } => {
                            // TODO: Are there any link types that would change how
                            // the image is rendered?

                            if builder_dirty {
                                let text_layout = TextLayout::new_with_text(
                                    &current_text,
                                    attr_list
                                );
                                res.push(MarkdownContent::Text(text_layout));
                                attr_list = AttrsList::new(default_attrs);
                                current_text.clear();
                                pos = 0;
                                builder_dirty = false;
                            }

                            res.push(MarkdownContent::Image {
                                url:   dest.to_string(),
                                title: title.to_string()
                            });
                        },
                        _ => {
                            // Presumably?
                            builder_dirty = true;
                        }
                    }
                } else {
                    log::warn!("Unbalanced markdown tag")
                }
            },
            Event::Text(text) => {
                if let Some((_, tag)) = tag_stack.last() {
                    if should_skip_text_in_tag(tag) {
                        continue;
                    }
                }
                current_text.push_str(&text);
                pos += text.len();
                last_text = text;
                builder_dirty = true;
            },
            Event::Code(text) => {
                attr_list.add_span(
                    pos..pos + text.len(),
                    default_attrs.family(&code_font_family)
                );
                current_text.push_str(&text);
                pos += text.len();
                builder_dirty = true;
            },
            // TODO: Some minimal 'parsing' of html could be useful here, since some
            // things use basic html like `<code>text</code>`.
            Event::Html(text) => {
                attr_list.add_span(
                    pos..pos + text.len(),
                    default_attrs
                        .family(&code_font_family)
                        .color(markdown_blockquote)
                );
                current_text.push_str(&text);
                pos += text.len();
                builder_dirty = true;
            },
            Event::HardBreak => {
                current_text.push('\n');
                pos += 1;
                builder_dirty = true;
            },
            Event::SoftBreak => {
                current_text.push(' ');
                pos += 1;
                builder_dirty = true;
            },
            Event::Rule => {},
            Event::FootnoteReference(_text) => {},
            Event::TaskListMarker(_text) => {},
            Event::InlineHtml(_) => {}, // TODO(panekj): Implement
            Event::InlineMath(_) => {}, // TODO(panekj): Implement
            Event::DisplayMath(_) => {} // TODO(panekj): Implement
        }
    }

    if builder_dirty {
        let text_layout = TextLayout::new_with_text(&current_text, attr_list);
        res.push(MarkdownContent::Text(text_layout));
    }

    res
}

fn attribute_for_tag<'a>(
    default_attrs: Attrs<'a>,
    tag: &Tag,
    code_font_family: &'a [FamilyOwned],
    font_size: f32,
    markdown_blockquote: Color,
    editor_link: Color
) -> Option<Attrs<'a>> {
    use pulldown_cmark::HeadingLevel;
    match tag {
        Tag::Heading {
            level,
            id: _,
            classes: _,
            attrs: _
        } => {
            // The size calculations are based on the em values given at
            // https://drafts.csswg.org/css2/#html-stylesheet
            let font_scale = match level {
                HeadingLevel::H1 => 2.0f32,
                HeadingLevel::H2 => 1.5,
                HeadingLevel::H3 => 1.17,
                HeadingLevel::H4 => 1.0,
                HeadingLevel::H5 => 0.83,
                HeadingLevel::H6 => 0.75
            };
            let font_size = font_scale * font_size;
            Some(default_attrs.font_size(font_size).weight(Weight::BOLD))
        },
        Tag::BlockQuote(_block_quote) => Some(
            default_attrs
                .style(Style::Italic)
                .color(markdown_blockquote)
        ),
        Tag::CodeBlock(_) => Some(default_attrs.family(code_font_family)),
        Tag::Emphasis => Some(default_attrs.style(Style::Italic)),
        Tag::Strong => Some(default_attrs.weight(Weight::BOLD)),
        // TODO: Strikethrough support
        Tag::Link {
            link_type: _,
            dest_url: _,
            title: _,
            id: _
        } => {
            // TODO: Link support
            Some(default_attrs.color(editor_link))
        },
        // All other tags are currently ignored
        _ => None
    }
}

/// Decides whether newlines should be added after a specific markdown tag
fn should_add_newline_after_tag(tag: &Tag) -> bool {
    !matches!(
        tag,
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. }
    )
}

/// Whether it should skip the text node after a specific tag  
/// For example, images are skipped because it emits their title as a separate
/// text node.
fn should_skip_text_in_tag(tag: &Tag) -> bool {
    matches!(tag, Tag::Image { .. })
}

fn md_language_to_lapce_language(lang: &str) -> Option<LapceLanguage> {
    // TODO: There are many other names commonly used that should be supported
    LapceLanguage::from_name(lang)
}

/// Highlight the text in a richtext builder like it was a markdown codeblock
pub fn highlight_as_code(
    attr_list: &mut AttrsList,
    default_attrs: Attrs,
    language: Option<LapceLanguage>,
    text: &str,
    start_offset: usize,
    directory: &Directory,
    style_colors: &HashMap<String, Color>
) {
    let syntax = language.map(|x| {
        Syntax::from_language(
            x,
            &directory.grammars_directory,
            &directory.queries_directory
        )
    });

    let styles = syntax
        .map(|mut syntax| {
            syntax.parse(
                0,
                Rope::from(text),
                None,
                &directory.grammars_directory,
                &directory.queries_directory
            );
            syntax.styles
        })
        .unwrap_or(None);

    if let Some(styles) = styles {
        for (range, fg) in styles.iter() {
            if let Some(color) = style_colors.get(fg) {
                attr_list.add_span(
                    start_offset + range.start..start_offset + range.end,
                    default_attrs.color(*color)
                );
            } else {
                warn!("fg {} is not found color", fg);
            }
        }
    }
}

pub fn from_marked_string(
    text: MarkedString,
    directory: &Directory,
    font_family: &str,
    editor_fg: Color,
    style_colors: &HashMap<String, Color>,
    font_size: f32,
    markdown_blockquote: Color,
    editor_link: Color
) -> Vec<MarkdownContent> {
    match text {
        MarkedString::String(text) => parse_markdown(
            &text,
            1.8,
            directory,
            font_family,
            editor_fg,
            style_colors,
            font_size,
            markdown_blockquote,
            editor_link
        ),
        // This is a short version of a code block
        MarkedString::LanguageString(code) => {
            // TODO: We could simply construct the MarkdownText directly
            // Simply construct the string as if it was written directly
            parse_markdown(
                &format!("```{}\n{}\n```", code.language, code.value),
                1.8,
                directory,
                font_family,
                editor_fg,
                style_colors,
                font_size,
                markdown_blockquote,
                editor_link
            )
        }
    }
}

pub fn from_plaintext(
    text: &str,
    line_height: f64,
    font_size: f32
) -> Vec<MarkdownContent> {
    let text_layout = TextLayout::new_with_text(
        text,
        AttrsList::new(
            Attrs::new()
                .font_size(font_size)
                .line_height(LineHeightValue::Normal(line_height as f32))
        )
    );
    vec![MarkdownContent::Text(text_layout)]
}
