use std::str;

pub const SCOPES: &[&str] = &[
    "constant",
    "type",
    "type.builtin",
    "property",
    "comment",
    "constructor",
    "function",
    "label",
    "keyword",
    "string",
    "variable",
    "variable.other.member",
    "operator",
    "attribute",
    "escape",
    "embedded",
    "symbol",
    "punctuation",
    "punctuation.special",
    "punctuation.delimiter",
    "text",
    "text.literal",
    "text.title",
    "text.uri",
    "text.reference",
    "string.escape",
    "conceal",
    "none",
    "tag",
    "markup.bold",
    "markup.italic",
    "markup.list",
    "markup.quote",
    "markup.heading",
    "markup.link.url",
    "markup.link.label",
    "markup.link.text",
];

// pub fn line_styles(
//     text: &Rope,
//     line: usize,
//     styles: &Spans<Style>,
// ) -> Vec<LineStyle> {
//     let max_line = text.measure::<LinesMetric>() + 1;
//
//     if line >= max_line {
//         return Vec::new();
//     }
//
//     let start_offset = text.offset_of_line(line);
//     let end_offset = text.offset_of_line(line + 1);
//     let line_str = text.slice_to_cow(start_offset..end_offset);
//     let line_styles: Vec<LineStyle> = styles
//         .iter_chunks(start_offset..end_offset)
//         .filter_map(|(iv, style)| {
//             let start = iv.start();
//             let end = iv.end();
//             if start > end_offset || end < start_offset {
//                 None
//             } else {
//                 let start = if start > start_offset {
//                     start - start_offset
//                 } else {
//                     0
//                 };
//                 let end = end - start_offset;
//                 let style = style.clone();
//                 let text = line_str.get(start..end).map(|x| x.to_string());
//                 Some(LineStyle {
//                     start,
//                     end,
//                     style,
//                     text,
//                 })
//             }
//         })
//         .collect();
//     line_styles
// }
