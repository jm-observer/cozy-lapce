use crate::lines_util::{
    check_line_final_col, check_lines_col, folded_v1, folded_v2, init_main_2
};

mod lines_util;

#[test]
fn test_folded_line_1() {
    custom_utils::logger::logger_stdout_debug();
    let mut _lines = init_main_2().unwrap();
    _lines.update_folding_ranges(folded_v1().into()).unwrap();
    // _lines.update_folding_ranges(folded_v2().into()).unwrap();
    {
        let text_layout = _lines.text_layout_of_visual_line(1).unwrap();
        check_lines_col(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    if true {\r\n    } else {\r\n",
            "    if true {...} else {\r\n"
        );
        check_line_final_col(
            &text_layout.phantom_text,
            "    if true {...} else {\r\n"
        );
    }
    {
        // let let_line = &_lines.visual_lines[4];
        // debug!("{:?}", let_line);
        let expect_str = "    let a: A  = A;\r\n";
        let text_layout = _lines.text_layout_of_visual_line(4).unwrap();

        check_lines_col(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    let a = A;\r\n",
            expect_str
        );
        check_line_final_col(&text_layout.phantom_text, expect_str);
    }
}

#[test]
fn test_folded_line_1_5() {
    let mut _lines = init_main_2().unwrap();
    _lines.update_folding_ranges(folded_v1().into()).unwrap();
    _lines.update_folding_ranges(folded_v2().into()).unwrap();
    {
        let text_layout = _lines.text_layout_of_visual_line(1).unwrap();
        check_lines_col(
            &text_layout.phantom_text.text,
            text_layout.phantom_text.final_text_len,
            "    if true {\r\n    } else {\r\n    }\r\n",
            "    if true {...} else {...}\r\n"
        );
        check_line_final_col(
            &text_layout.phantom_text,
            "    if true {...} else {...}\r\n"
        );
    }
}
