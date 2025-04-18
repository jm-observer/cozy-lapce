use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{self},
    },
};

use doc::{
    diagnostic::DiagnosticData,
    language::LapceLanguage,
    lines::{
        DocLinesManager, EditBuffer, RopeTextPosition,
        buffer::{
            Buffer, InvalLines,
            diff::DiffLines,
            rope_text::{RopeText, RopeTextVal},
        },
        char_buffer::CharBuffer,
        command::EditCommand,
        cursor::Cursor,
        edit::EditType,
        line_ending::LineEnding,
        mode::MotionMode,
        register::Register,
        screen_lines::ScreenLines,
        selection::{InsertDrift, Selection},
        style::EditorStyle,
        text::PreeditData,
        word::WordCursor,
    },
    syntax::{BracketParser, Syntax, edit::SyntaxEdit},
};
use floem::{
    ext_event::create_ext_action,
    kurbo::Rect,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, batch},
    text::FamilyOwned,
};
use itertools::Itertools;
use lapce_core::{doc::DocContent, workspace::LapceWorkspace};
use lapce_rpc::{buffer::BufferId, plugin::PluginId, proxy::ProxyResponse};
use lapce_xi_rope::{Interval, Rope, RopeDelta, spans::SpansBuilder};
use log::{debug, error};
use lsp_types::{CodeLens, Diagnostic, DocumentSymbolResponse};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{
    command::InternalCommand,
    editor::{
        floem_editor::CommonAction,
        location::{EditorLocation, EditorPosition},
    },
    find::{Find, FindProgress, FindResult},
    history::DocumentHistory,
    local_task::{LocalRequest, LocalResponse},
    panel::document_symbol::{
        DocumentSymbolViewData, SymbolData, SymbolInformationItemData,
    },
    window_workspace::CommonData,
};
// #[derive(Clone, Debug)]
// pub struct DiagnosticData {
//     pub expanded: RwSignal<bool>,
//     pub diagnostics: RwSignal<im::Vector<Diagnostic>>,
//     pub diagnostics_span: RwSignal<Spans<Diagnostic>>,
// }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub range:      Option<(usize, usize)>,
    pub diagnostic: Diagnostic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocInfo {
    pub workspace:     LapceWorkspace,
    pub path:          PathBuf,
    pub scroll_offset: (f64, f64),
    pub cursor_offset: usize,
}

pub type AllCodeLens = im::HashMap<usize, (PluginId, usize, im::Vector<CodeLens>)>;

#[derive(Clone)]
pub struct Doc {
    pub name:      Option<String>,
    pub scope:     Scope,
    pub buffer_id: BufferId,
    pub content:   RwSignal<DocContent>,
    pub cache_rev: RwSignal<u64>,
    /// Whether the buffer's content has been loaded/initialized into the
    /// buffer.
    pub loaded:    RwSignal<bool>,
    // pub kind: RwSignal<EditorViewKind>,
    pub code_lens: RwSignal<AllCodeLens>,

    /// Stores information about different versions of the document from source
    /// control.
    histories:        RwSignal<im::HashMap<String, DocumentHistory>>,
    pub head_changes: RwSignal<im::Vector<DiffLines>>,

    /// A cache for the sticky headers which maps a line to the lines it should
    /// show in the header.
    pub sticky_headers: Rc<RefCell<HashMap<usize, Option<Vec<usize>>>>>,

    pub find_result: FindResult,

    // editors:    Editors,
    pub common: Rc<CommonData>,

    pub document_symbol_data: DocumentSymbolViewData,

    pub lines: DocLinesManager, // pub screen_lines: RwSignal<ScreenLines>,
}

impl Doc {
    pub fn new(
        cx: Scope,
        path: PathBuf,
        diagnostics: DiagnosticData,
        common: Rc<CommonData>,
        doc_content: DocContent,
    ) -> Self {
        let queries_directory = common.directory.queries_directory.clone();
        let grammars_directory = common.directory.grammars_directory.clone();
        let syntax = Syntax::init(&path, &grammars_directory, &queries_directory);
        let (rw_config, bracket_pair_colorization, bracket_colorization_limit) =
            common.config.with_untracked(|config| {
                (
                    config.get_doc_editor_config(),
                    config.editor.bracket_pair_colorization,
                    config.editor.bracket_colorization_limit,
                )
            });
        let viewport = Rect::ZERO;
        let editor_style = EditorStyle::default();
        let buffer = Buffer::new("");
        // let kind = cx.create_rw_signal(EditorViewKind::Normal);
        let lines = DocLinesManager::new(
            cx,
            diagnostics,
            syntax,
            BracketParser::new(
                String::new(),
                bracket_pair_colorization,
                bracket_colorization_limit,
            ),
            viewport,
            editor_style,
            rw_config,
            buffer,
            // kind,
            Some(path.clone()),
        );
        let config = common.config;
        cx.create_effect(move |_| {
            let editor_config = config.with(|x| x.get_doc_editor_config());
            lines.update(|x| {
                if let Err(err) = x.update_config(editor_config) {
                    error!("{err:?}");
                }
            });
        });

        Doc {
            name: None,
            // kind,
            // viewport
            // editor_style,
            scope: cx,
            buffer_id: BufferId::next(),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(doc_content),
            loaded: cx.create_rw_signal(false),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            find_result: FindResult::new(cx),
            // preedit: PreeditData::new(cx),
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: DocumentSymbolViewData::new(cx),
            // folding_ranges: cx.create_rw_signal(FoldingRanges::default()),
            // semantic_previous_rs_id: cx.create_rw_signal(None),
            lines,
        }
    }

    pub fn new_local(
        cx: Scope,
        common: Rc<CommonData>,
        name: Option<String>,
    ) -> Doc {
        Self::new_content(cx, DocContent::Local, common, name)
    }

    pub fn new_content(
        cx: Scope,
        content: DocContent,
        common: Rc<CommonData>,
        name: Option<String>,
    ) -> Self {
        let cx = cx.create_child();
        let (rw_config, bracket_pair_colorization, bracket_colorization_limit) =
            common.config.with_untracked(|config| {
                (
                    config.get_doc_editor_config(),
                    config.editor.bracket_pair_colorization,
                    config.editor.bracket_colorization_limit,
                )
            });
        let viewport = Rect::ZERO;
        let editor_style = EditorStyle::default();
        let diagnostics = DiagnosticData::new(cx);
        let syntax = Syntax::plaintext(
            &common.directory.grammars_directory,
            &common.directory.queries_directory,
        );
        let buffer = Buffer::new("");
        // let kind = cx.create_rw_signal(EditorViewKind::Normal);

        let lines = DocLinesManager::new(
            cx,
            diagnostics,
            syntax,
            BracketParser::new(
                String::new(),
                bracket_pair_colorization,
                bracket_colorization_limit,
            ),
            viewport,
            editor_style,
            rw_config,
            buffer,
            None,
        );
        let config = common.config;
        cx.create_effect(move |_| {
            let editor_config = config.with(|x| x.get_doc_editor_config());
            lines.update(|x| {
                if let Err(err) = x.update_config(editor_config) {
                    error!("{:?}", err);
                }
            });
        });
        Self {
            name,
            scope: cx,
            buffer_id: BufferId::next(),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(content),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            loaded: cx.create_rw_signal(true),
            find_result: FindResult::new(cx),
            // preedit: PreeditData::new(cx),
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: DocumentSymbolViewData::new(cx),
            lines,
        }
    }

    pub fn new_history(
        cx: Scope,
        content: DocContent,
        common: Rc<CommonData>,
    ) -> Self {
        let (rw_config, bracket_pair_colorization, bracket_colorization_limit) =
            common.config.with_untracked(|config| {
                (
                    config.get_doc_editor_config(),
                    config.editor.bracket_pair_colorization,
                    config.editor.bracket_colorization_limit,
                )
            });
        let syntax = if let DocContent::History(history) = &content {
            Syntax::init(
                &history.path,
                &common.directory.grammars_directory,
                &common.directory.queries_directory,
            )
        } else {
            Syntax::plaintext(
                &common.directory.grammars_directory,
                &common.directory.queries_directory,
            )
        };
        // let lines = cx.create_rw_signal(Lines::new(cx));
        let viewport = Rect::ZERO;
        let editor_style = EditorStyle::default();

        let diagnostics = DiagnosticData::new(cx);

        let buffer = Buffer::new("");
        // let kind = cx.create_rw_signal(EditorViewKind::Normal);

        let lines = DocLinesManager::new(
            cx,
            diagnostics,
            syntax,
            BracketParser::new(
                String::new(),
                bracket_pair_colorization,
                bracket_colorization_limit,
            ),
            viewport,
            editor_style,
            rw_config,
            buffer,
            None,
        );
        let config = common.config;
        cx.create_effect(move |_| {
            let editor_config = config.with(|x| x.get_doc_editor_config());
            lines.update(|x| {
                if let Err(err) = x.update_config(editor_config) {
                    error!("{:?}", err);
                }
            });
        });

        Self {
            name: None,
            scope: cx,
            buffer_id: BufferId::next(),
            // syntax: cx.create_rw_signal(syntax),
            // line_styles: Rc::new(RefCell::new(HashMap::new())),
            // semantic_styles: cx.create_rw_signal(None),
            // inlay_hints: cx.create_rw_signal(None),
            // completion_lens: cx.create_rw_signal(None),
            // completion_pos: cx.create_rw_signal((0, 0)),
            // inline_completion: cx.create_rw_signal(None),
            // inline_completion_pos: cx.create_rw_signal((0, 0)),
            cache_rev: cx.create_rw_signal(0),
            content: cx.create_rw_signal(content),
            sticky_headers: Rc::new(RefCell::new(HashMap::new())),
            loaded: cx.create_rw_signal(false),
            histories: cx.create_rw_signal(im::HashMap::new()),
            head_changes: cx.create_rw_signal(im::Vector::new()),
            find_result: FindResult::new(cx),
            // preedit: PreeditData::new(cx),
            common,
            code_lens: cx.create_rw_signal(im::HashMap::new()),
            document_symbol_data: DocumentSymbolViewData::new(cx),
            // folding_ranges: cx.create_rw_signal(FoldingRanges::default()),
            // semantic_previous_rs_id: cx.create_rw_signal(None),
            // lines,
            // viewport,
            // editor_style,
            lines,
        }
    }

    pub fn buffer_edit(
        &self,
        edit: EditBuffer,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        self.buffer_edit_with_config(edit, true)
    }

    pub fn buffer_edit_with_config(
        &self,
        edit: EditBuffer,
        need_check_and_update: bool,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        let Some((old_rev, rs, new_rev)) = self.lines.try_update(|lines| {
            let rev = lines.buffer().rev();
            let rs = lines._buffer_edit(edit);
            let new_rev = lines.buffer().rev();
            (rev, rs, new_rev)
        }) else {
            error!("buffer_edit_with_config try update none??!");
            return vec![];
        };
        if need_check_and_update {
            assert_eq!(old_rev + rs.len() as u64, new_rev);
            let content = self.content.get_untracked();
            log::warn!(
                "buffer_edit_with_config update {content:?} old_rev={old_rev} \
                 new_rev={new_rev}",
            );
            if let DocContent::File { path, .. } = content {
                batch(|| {
                    for (i, (_, delta, _inval)) in rs.iter().enumerate() {
                        self.common.proxy.proxy_rpc.update(
                            path.clone(),
                            delta.clone(),
                            old_rev + i as u64 + 1,
                        );
                    }
                });
            }
        }
        rs
    }

    pub fn syntax(&self) -> Syntax {
        self.lines.with_untracked(|x| x.syntax.clone())
    }

    pub fn set_syntax(&self, syntax: Syntax) {
        batch(|| {
            self.lines.update(|x| {
                if let Err(err) = x.set_syntax(syntax) {
                    error!("{:?}", err);
                }
            });
            // {
            //
            // }
            self.clear_text_cache();
            self.clear_sticky_headers_cache();
        });
    }

    /// Set the syntax highlighting this document should use.
    pub fn set_language(&self, language: LapceLanguage) {
        self.lines.update(|x| {
            if let Err(err) = x.set_syntax(Syntax::from_language(
                language,
                &self.common.directory.grammars_directory,
                &self.common.directory.queries_directory,
            )) {
                error!("{:?}", err);
            }
        });
    }

    pub fn find(&self) -> &Find {
        &self.common.find
    }

    /// Whether or not the underlying buffer is loaded
    pub fn loaded(&self) -> bool {
        self.loaded.get_untracked()
    }

    //// Initialize the content with some text, this marks the document as loaded.
    pub fn init_content(&self, content: Rope) {
        batch(|| {
            self.buffer_edit_with_config(EditBuffer::Init(content), false);
            self.loaded.set(true);
            self.on_update(None);
            self.retrieve_head();
        });
    }

    /// Reload the document's content, and is what you should typically use when
    /// you want to *set* an existing document's content.
    pub fn reload(&self, content: Rope, set_pristine: bool) {
        // self.code_actions.clear();
        // self.inlay_hints = None;
        let delta = self
            .buffer_edit(EditBuffer::Reload {
                content,
                set_pristine,
            })
            .remove(0);
        self.apply_deltas(&[delta]);
    }

    pub fn handle_file_changed(&self, content: Rope) {
        if self.is_pristine() {
            self.reload(content, true);
        }
    }

    pub fn do_insert(
        &self,
        cursor: &mut Cursor,
        s: &str,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        if self.content.with_untracked(|c| c.read_only()) {
            return Vec::new();
        }
        // let Some(deltas) = self
        //     .lines
        //     .try_update(|lines| lines.do_insert_buffer(cursor, s))
        // else {
        //     return vec![];
        // };
        let deltas = self.buffer_edit(EditBuffer::DoInsertBuffer { cursor, s });
        self.apply_deltas(&deltas);
        deltas
    }

    pub fn do_raw_edit(
        &self,
        edits: &[(Selection, &str)],
        edit_type: EditType,
    ) -> Option<(Rope, RopeDelta, InvalLines)> {
        if self.content.with_untracked(|c| c.read_only()) {
            return None;
        }

        let (text, delta, inval_lines) = self
            .buffer_edit(EditBuffer::EditBuffer {
                edit_type,
                iter: edits,
            })
            .remove(0);
        // let (text, delta, inval_lines) = self
        //     .lines
        //     .try_update(|buffer| buffer.edit_buffer(edits, edit_type))?;
        self.apply_deltas(&[(text.clone(), delta.clone(), inval_lines.clone())]);
        Some((text, delta, inval_lines))
    }

    pub fn do_edit(
        &self,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
        screen_lines: Arc<ScreenLines>,
    ) -> Vec<(Rope, RopeDelta, InvalLines)> {
        if self.content.with_untracked(|c| c.read_only())
            && !cmd.not_changing_buffer()
        {
            debug!("do_edit read_only or not_changing_buffer");
            return Vec::new();
        }

        let deltas = self.buffer_edit(EditBuffer::DoEditBuffer {
            cursor,
            cmd,
            modal,
            register,
            smart_tab,
            screen_lines,
        });
        if !deltas.is_empty() {
            self.apply_deltas(&deltas);
        }

        deltas
    }

    pub fn apply_deltas(&self, deltas: &[(Rope, RopeDelta, InvalLines)]) {
        if let DocContent::File { path, .. } = self.content.get_untracked() {
            batch(|| {
                for (_, delta, inval) in deltas.iter() {
                    // self.apply_deltas_for_lines(delta);
                    self.update_find_result(delta);
                    self.update_breakpoints(delta, &path, &inval.old_text);
                }
            });
        }
        // TODO(minor): We could avoid this potential allocation since most
        // apply_delta callers are actually using a Vec which we could reuse.
        // We use a smallvec because there is unlikely to be more than a couple of
        // deltas
        let edits: SmallVec<[SyntaxEdit; 3]> = deltas
            .iter()
            .filter_map(|(before_text, delta, _)| {
                match SyntaxEdit::from_delta(before_text, delta.clone()) {
                    Ok(rs) => Some(rs),
                    Err(err) => {
                        error!("{err:?}");
                        None
                    },
                }
            })
            .collect();
        self.on_update(Some(edits));
    }

    pub fn is_pristine(&self) -> bool {
        self.lines.with_untracked(|b| b.buffer().is_pristine())
    }

    /// Get the buffer's current revision. This is used to track whether the
    /// buffer has changed.
    pub fn rev(&self) -> u64 {
        self.lines.with_untracked(|b| b.buffer().rev())
    }

    /// Get the buffer's line-ending.
    /// Note: this may not be the same as what the actual line endings in the
    /// file are, rather this is what the line-ending is set to (and what it
    /// will be saved as).
    pub fn line_ending(&self) -> LineEnding {
        self.lines.with_untracked(|b| b.buffer().line_ending())
    }

    fn on_update(&self, edits: Option<SmallVec<[SyntaxEdit; 3]>>) {
        if self.content.get_untracked().is_local() {
            debug!("on_update cancle because doc is local");
            batch(|| {
                self.clear_text_cache();
                // self.lines.update(|x| x.on_update_buffer());
            });
            return;
        }
        batch(|| {
            self.trigger_syntax_change(edits);
            self.trigger_head_change();
            self.get_inlay_hints();
            self.find_result.reset();
            self.get_semantic_styles();
            // self.do_bracket_colorization();
            self.clear_style_cache();
            self.get_code_lens();
            self.get_document_symbol();
            self.get_folding_range();
            // self.lines.update(|x| x.on_update_buffer());
        });
    }

    // fn do_bracket_colorization(&self) {
    //     self.lines.update(|x| x.do_bracket_colorization());
    // }

    // pub fn do_text_edit(&self, edits: &[TextEdit]) {
    //     let edits = self.buffer.with_untracked(|buffer| {
    //         let edits = edits
    //             .iter()
    //             .map(|edit| {
    //                 let selection = lapce_core::selection::Selection::region(
    //                     buffer.offset_of_position(&edit.range.start),
    //                     buffer.offset_of_position(&edit.range.end),
    //                 );
    //                 (selection, edit.new_text.as_str())
    //             })
    //             .collect::<Vec<_>>();
    //         edits
    //     });
    //     self.do_raw_edit(&edits, EditType::Completion);
    // }

    // /// Update the styles after an edit, so the highlights are at the correct
    // positions. /// This does not do a reparse of the document itself.
    // fn apply_deltas_for_lines(&self, delta: &RopeDelta) {
    //     self.lines.update(|x| x.apply_delta(delta));
    // }

    pub fn trigger_syntax_change(&self, edits: Option<SmallVec<[SyntaxEdit; 3]>>) {
        let (rev, text) = self
            .lines
            .with_untracked(|b| (b.buffer().rev(), b.buffer().text().clone()));

        let doc = self.clone();
        let send = create_ext_action(self.scope, move |syntax| {
            match doc.lines.try_update(|x| x.set_syntax_with_rev(syntax, rev)) {
                Some(Ok(rs)) => {
                    if rs {
                        // doc.do_bracket_colorization();
                        doc.clear_sticky_headers_cache();
                        doc.clear_text_cache();
                    }
                },
                Some(Err(err)) => {
                    error!("{err:?}");
                },
                None => {
                    error!("None");
                },
            };
        });

        self.lines.update(|x| {
            if let Err(err) = x.trigger_syntax_change(edits.clone()) {
                error!("{err:?}");
            }
        });
        let syntax = self.syntax();
        // let grammars_directory = self.common.directory.grammars_directory.clone();
        // let queries_directory = self.common.directory.queries_directory.clone();

        self.common.local_task.request_async(
            LocalRequest::SyntaxParse {
                rev,
                text,
                edits,
                syntax,
            },
            move |(_id, rs)| match rs {
                Ok(response) => {
                    if let LocalResponse::SyntaxParse { syntax } = response {
                        send(syntax);
                    }
                },
                Err(err) => {
                    error!("{err:?}")
                },
            },
        );
        // rayon::spawn(move || {
        //     syntax.parse(
        //         rev,
        //         text,
        //         edits.as_deref(),
        //         &grammars_directory,
        //         &queries_directory,
        //     );
        //     send(syntax);
        // });
    }

    fn clear_style_cache(&self) {
        self.clear_text_cache();
    }

    /// Inform any dependents on this document that they should clear any cached
    /// text.
    pub fn clear_text_cache(&self) {
        self.cache_rev.try_update(|cache_rev| {
            *cache_rev += 1;
            // TODO: ???
            // Update the text layouts within the callback so that those alerted
            // to cache rev will see the now empty layouts.
            // self.text_layouts.borrow_mut().clear(*cache_rev, None);
        });
    }

    fn clear_sticky_headers_cache(&self) {
        self.sticky_headers.borrow_mut().clear();
    }

    /// Request semantic styles for the buffer from the LSP through the proxy.
    // pub fn get_semantic_styles(&self) {
    pub fn get_semantic_full_styles(&self) {
        if !self.loaded() {
            return;
        }
        let path =
            if let DocContent::File { path, .. } = self.content.get_untracked() {
                path
            } else {
                return;
            };

        let (atomic_rev, rev, len) = self.lines.with_untracked(|b| {
            (b.buffer().atomic_rev(), b.buffer().rev(), b.buffer().len())
        });

        let doc = self.clone();
        let atomic_rev_clone = atomic_rev.clone();
        let send = create_ext_action(self.scope, move |(styles, result_id)| {
            if atomic_rev_clone.load(atomic::Ordering::Acquire) != rev {
                return;
            }
            if let Some(styles) = styles {
                // error!("{:?}", styles);
                match doc.lines.try_update(|x| {
                    x.update_semantic_styles_from_lsp((result_id, styles), rev)
                }) {
                    Some(Ok(true)) => {
                        doc.clear_style_cache();
                    },
                    Some(Err(err)) => {
                        error!("{err:?}");
                    },
                    _ => {},
                }
            }
        });
        let local_task = self.common.local_task.clone();
        self.common
            .proxy
            .proxy_rpc
            .get_semantic_tokens(path, move |(_, result)| {
                if let Ok(ProxyResponse::GetSemanticTokens { styles, result_id }) =
                    result
                {
                    if styles.styles.is_empty() {
                        send((None, result_id));
                        return;
                    }
                    if atomic_rev.load(atomic::Ordering::Acquire) != rev {
                        return;
                    }
                    local_task.request_async(
                        LocalRequest::SpansBuilder {
                            styles,
                            result_id,
                            len,
                        },
                        move |(_id, rs)| match rs {
                            Ok(response) => {
                                if let LocalResponse::SpansBuilder {
                                    styles,
                                    result_id,
                                } = response
                                {
                                    send((Some(styles), result_id));
                                }
                            },
                            Err(err) => {
                                error!("{err:?}")
                            },
                        },
                    );
                } else {
                    send((None, None));
                }
            });
    }

    /// Request semantic styles for the buffer from the LSP through the proxy.
    pub fn get_semantic_styles(&self) {
        // todo
        self.get_semantic_full_styles();
    }

    pub fn get_code_lens(&self) {
        let cx = self.scope;
        let doc = self.clone();
        self.code_lens.update(|code_lens| {
            code_lens.clear();
        });
        let rev = self.rev();
        if let DocContent::File { path, .. } = doc.content.get_untracked() {
            let send = create_ext_action(cx, move |result| {
                if rev != doc.rev() {
                    return;
                }
                if let Ok(ProxyResponse::GetCodeLensResponse { plugin_id, resp }) =
                    result
                {
                    let Some(codelens) = resp else {
                        return;
                    };
                    doc.code_lens.update(|code_lens| {
                        for codelens in codelens {
                            if codelens.command.is_none() {
                                continue;
                            }
                            let rs = match doc.lines.with_untracked(|b| {
                                b.buffer().offset_of_line(
                                    codelens.range.start.line as usize,
                                )
                            }) {
                                Ok(rs) => rs,
                                Err(err) => {
                                    error!("{err:?}");
                                    continue;
                                },
                            };
                            let entry = code_lens
                                .entry(codelens.range.start.line as usize)
                                .or_insert_with(|| {
                                    (plugin_id, rs, im::Vector::new())
                                });
                            entry.2.push_back(codelens);
                        }
                    });
                }
            });
            self.common
                .proxy
                .proxy_rpc
                .get_code_lens(path, move |(_, result)| {
                    send(result);
                });
        }
    }

    pub fn get_document_symbol(&self) {
        let cx = self.scope;
        let doc = self.clone();
        let rev = self.rev();
        if let DocContent::File { path, .. } = doc.content.get_untracked() {
            let send = create_ext_action(cx, {
                let path = path.clone();
                move |result| {
                    if rev != doc.rev() {
                        return;
                    }
                    if let Ok(ProxyResponse::GetDocumentSymbols { resp }) = result {
                        let items: Vec<RwSignal<SymbolInformationItemData>> =
                            match resp {
                                DocumentSymbolResponse::Flat(_symbols) => {
                                    Vec::with_capacity(0)
                                },
                                DocumentSymbolResponse::Nested(symbols) => symbols
                                    .into_iter()
                                    .map(|x| {
                                        cx.create_rw_signal(
                                            SymbolInformationItemData::from((x, cx)),
                                        )
                                    })
                                    .collect(),
                            };
                        let symbol_new = Some(SymbolData::new(items, path, cx));
                        doc.document_symbol_data.virtual_list.update(|symbol| {
                            symbol.update(symbol_new);
                        });
                    }
                }
            });

            self.common.proxy.proxy_rpc.get_document_symbols(
                path,
                move |(_, result)| {
                    send(result);
                },
            );
        }
    }

    /// Request inlay hints for the buffer from the LSP through the proxy.
    pub fn get_inlay_hints(&self) {
        if !self.loaded() {
            return;
        }

        let path =
            if let DocContent::File { path, .. } = self.content.get_untracked() {
                path
            } else {
                return;
            };

        let (buffer, rev, len) = self.lines.with_untracked(|b| {
            (b.buffer().clone(), b.buffer().rev(), b.buffer().len())
        });

        let doc = self.clone();
        let send = create_ext_action(self.scope, move |hints| {
            if let Some(true) = doc.lines.try_update(|x| {
                if x.buffer().rev() == rev {
                    if let Err(err) = x.set_inlay_hints(hints) {
                        error!("{err:?}");
                    }
                    true
                } else {
                    false
                }
            }) {
                doc.clear_text_cache();
            }
        });

        self.common
            .proxy
            .proxy_rpc
            .get_inlay_hints(path, move |(_, result)| {
                if let Ok(ProxyResponse::GetInlayHints { mut hints }) = result {
                    // log::info!("{}", serde_json::to_string(&hints).unwrap());
                    // Sort the inlay hints by their position, as the LSP does not
                    // guarantee that it will provide them in the order
                    // that they are in within the file as well, Spans
                    // does not iterate in the order that they appear
                    hints.sort_by(|left, right| left.position.cmp(&right.position));

                    let mut hints_span = SpansBuilder::new(len);
                    for hint in hints {
                        let offset = match buffer.offset_of_position(&hint.position)
                        {
                            Ok(rs) => rs,
                            Err(err) => {
                                error!("{err:?}");
                                continue;
                            },
                        }
                        .min(len);
                        hints_span.add_span(
                            Interval::new(offset, (offset + 1).min(len)),
                            hint,
                        );
                    }
                    let hints = hints_span.build();
                    send(hints);
                }
            });
    }

    pub fn diagnostics(&self) -> DiagnosticData {
        self.lines.with_untracked(|x| x.diagnostics)
    }

    // /// Update the diagnostics' positions after an edit so that they appear in
    // the correct place. fn update_diagnostics(&self, delta: &RopeDelta) {
    //     self.lines.update(|x| );
    // }

    /// init diagnostics offset ranges from lsp positions
    pub fn init_diagnostics(&self) {
        batch(|| {
            self.clear_text_cache();
            self.lines.update(|x| {
                if let Err(err) = x.init_diagnostics() {
                    error!("{err:?}");
                }
            });
        });
    }

    pub fn get_folding_range(&self) {
        let cx = self.scope;
        let doc = self.clone();
        let rev = self.rev();
        if let DocContent::File { path, .. } = doc.content.get_untracked() {
            let send = create_ext_action(cx, {
                move |result| {
                    if rev != doc.rev() {
                        return;
                    }
                    if let Ok(ProxyResponse::LspFoldingRangeResponse {
                        resp, ..
                    }) = result
                    {
                        let folding: Vec<lsp_types::FoldingRange> = resp
                            .unwrap_or_default()
                            .into_iter()
                            .sorted_by(|x, y| x.start_line.cmp(&y.start_line))
                            .collect();
                        doc.lines.update(|symbol| {
                            if let Err(err) =
                                symbol.update_folding_ranges(folding.into())
                            {
                                error!("{err:?}");
                            }
                        });
                        doc.clear_text_cache();
                    }
                }
            });

            self.common.proxy.proxy_rpc.get_lsp_folding_range(
                path,
                move |(_, result)| {
                    send(result);
                },
            );
        }
    }

    /// Get the current completion lens text
    pub fn completion_lens(&self) -> Option<String> {
        self.lines.with_untracked(|x| x.completion_lens.clone())
    }

    pub fn set_completion_lens(
        &self,
        completion_lens: String,
        line: usize,
        col: usize,
    ) {
        self.lines.update(|x| {
            if let Err(err) = x.set_completion_lens(completion_lens, line, col) {
                error!("{err:?}");
            }
        });
    }

    pub fn clear_completion_lens(&self) {
        self.lines.update(|x| x.clear_completion_lens());
    }

    fn update_breakpoints(&self, delta: &RopeDelta, path: &Path, old_text: &Rope) {
        if self.common.breakpoints.contains_path(path) {
            self.common
                .breakpoints
                .update_by_rope_delta(delta, path, old_text, self.lines);
        }
    }

    // /// Update the completion lens position after an edit so that it appears in
    // the correct place. pub fn update_completion_lens(&self, delta:
    // &RopeDelta) {     self.lines.update(|x| x.update_completion_lens(delta));
    // }

    fn update_find_result(&self, delta: &RopeDelta) {
        self.find_result.occurrences.update(|s| {
            *s = s.apply_delta(delta, true, InsertDrift::Default);
        })
    }

    pub fn update_find(&self) {
        let find_rev = self.common.find.rev.get_untracked();
        if self.find_result.find_rev.get_untracked() != find_rev {
            if self
                .common
                .find
                .search_string
                .with_untracked(|search_string| {
                    search_string
                        .as_ref()
                        .map(|s| s.content.is_empty())
                        .unwrap_or(true)
                })
            {
                self.find_result.occurrences.set(Selection::new());
            }
            self.find_result.reset();
            self.find_result.find_rev.set(find_rev);
        }

        if self.find_result.progress.get_untracked() != FindProgress::Started {
            return;
        }

        let search = self.common.find.search_string.get_untracked();
        let search = match search {
            Some(search) => search,
            None => return,
        };
        if search.content.is_empty() {
            return;
        }

        self.find_result
            .progress
            .set(FindProgress::InProgress(Selection::new()));

        let find_result = self.find_result.clone();
        let find_rev_signal = self.common.find.rev;
        let triggered_by_changes = self.common.find.triggered_by_changes;

        let path = self.content.get_untracked().path().cloned();
        let common = self.common.clone();
        let send = create_ext_action(self.scope, move |occurrences: Selection| {
            if let (false, Some(path), true, true) = (
                occurrences.regions().is_empty(),
                &path,
                find_rev_signal.get_untracked() == find_rev,
                triggered_by_changes.get_untracked(),
            ) {
                triggered_by_changes.set(false);
                common.internal_command.send(InternalCommand::GoToLocation {
                    location: EditorLocation {
                        path:               path.clone(),
                        position:           Some(EditorPosition::Offset(
                            occurrences.regions()[0].start,
                        )),
                        scroll_offset:      None,
                        ignore_unconfirmed: false,
                        same_editor_tab:    false,
                    },
                });
            }
            find_result.occurrences.set(occurrences);
            find_result.progress.set(FindProgress::Ready);
        });

        let text = self.lines.with_untracked(|b| b.buffer().text().clone());
        let case_matching = self.common.find.case_matching.get_untracked();
        let whole_words = self.common.find.whole_words.get_untracked();

        self.common.local_task.request_async(
            LocalRequest::FindText {
                text,
                case_matching,
                whole_words,
                search,
            },
            move |(_id, rs)| match rs {
                Ok(response) => {
                    if let LocalResponse::FindText { selection } = response {
                        send(selection);
                    }
                },
                Err(err) => {
                    error!("{err}")
                },
            },
        );
        // rayon::spawn(move || {
        //     let mut occurrences = Selection::new();
        //     Find::find(
        //         &text,
        //         &search,
        //         0,
        //         text.len(),
        //         case_matching,
        //         whole_words,
        //         true,
        //         &mut occurrences,
        //     );
        //     send(occurrences);
        // });
    }

    /// Get the sticky headers for a particular line, creating them if
    /// necessary.
    pub fn sticky_headers(&self, line: usize) -> Option<Vec<usize>> {
        if let Some(lines) = self.sticky_headers.borrow().get(&line) {
            return lines.clone();
        }
        let lines = self.lines.with_untracked(|buffer| {
            let buffer = buffer.buffer();
            let offset = match buffer.offset_of_line(line + 1) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return None;
                },
            };
            self.lines.with_untracked(|x| {
                x.syntax.sticky_headers(offset).map(|offsets| {
                    offsets
                        .iter()
                        .filter_map(|offset| {
                            let l = buffer.line_of_offset(*offset);
                            if l <= line { Some(l) } else { None }
                        })
                        .dedup()
                        .sorted()
                        .collect()
                })
            })
        });
        self.sticky_headers.borrow_mut().insert(line, lines.clone());
        lines
    }

    pub fn head_changes(&self) -> RwSignal<im::Vector<DiffLines>> {
        self.head_changes
    }

    /// Retrieve the `head` version of the buffer
    pub fn retrieve_head(&self) {
        if let DocContent::File { path, .. } = self.content.get_untracked() {
            let histories = self.histories;

            let send = {
                let path = path.clone();
                let doc = self.clone();
                create_ext_action(self.scope, move |result| {
                    if let Ok(ProxyResponse::BufferHeadResponse {
                        content, ..
                    }) = result
                    {
                        let hisotry = DocumentHistory::new(
                            path.clone(),
                            "head".to_string(),
                            &content,
                        );
                        histories.update(|histories| {
                            histories.insert("head".to_string(), hisotry);
                        });

                        doc.trigger_head_change();
                    }
                })
            };

            let path = path.clone();
            let proxy = self.common.proxy.clone();
            proxy.proxy_rpc.get_buffer_head(path, move |(_, result)| {
                send(result);
            });
        }
    }

    pub fn trigger_head_change(&self) {
        let history = if let Some(text) =
            self.histories.with_untracked(|histories| {
                histories
                    .get("head")
                    .map(|history| history.buffer.text().clone())
            }) {
            text
        } else {
            return;
        };

        let rev = self.rev();
        let left_rope = history;
        let (atomic_rev, right_rope) = self.lines.with_untracked(|b| {
            (b.buffer().atomic_rev(), b.buffer().text().clone())
        });

        let send = {
            let atomic_rev = atomic_rev.clone();
            let head_changes = self.head_changes;
            create_ext_action(self.scope, move |changes| {
                let changes = if let Some(changes) = changes {
                    changes
                } else {
                    return;
                };

                if atomic_rev.load(atomic::Ordering::Acquire) != rev {
                    return;
                }

                head_changes.set(changes);
            })
        };

        self.common.local_task.request_async(
            LocalRequest::RopeDiff {
                left_rope,
                right_rope,
                rev,
                atomic_rev,
                context_lines: None,
            },
            move |(_id, rs)| match rs {
                Ok(response) => {
                    if let LocalResponse::RopeDiff { changes, .. } = response {
                        send(changes.map(im::Vector::from));
                    }
                },
                Err(err) => {
                    error!("{err}")
                },
            },
        );

        // rayon::spawn(move || {
        //     let changes =
        //         rope_diff(left_rope, right_rope, rev, atomic_rev.clone(),
        // None);     send(changes.map(im::Vector::from));
        // });
    }

    pub fn save(&self, after_action: impl FnOnce() + 'static) {
        let content = self.content.get_untracked();
        if let DocContent::File { path, .. } = content {
            let rev = self.rev();
            // let buffer = self.lines.with_untracked(|x| x.signal_buffer());
            let doc = self.clone();

            let send = create_ext_action(self.scope, move |result| match result {
                Ok(_) => {
                    // lines.try_update(|x| x.set_pristine(rev));
                    doc.buffer_edit(EditBuffer::SetPristine(rev));
                    after_action();
                },
                Err(err) => error!("{err}"),
            });

            self.common
                .proxy
                .proxy_rpc
                .save(rev, path, true, move |(_, result)| {
                    send(result);
                })
        }
    }

    pub fn set_inline_completion(
        &self,
        inline_completion: String,
        line: usize,
        col: usize,
    ) {
        // TODO: more granular invalidation
        batch(|| {
            self.lines.update(|x| {
                if let Err(err) =
                    x.set_inline_completion(inline_completion, line, col)
                {
                    error!("{err:?}");
                }
            });
            self.clear_text_cache();
        });
    }

    pub fn clear_inline_completion(&self) {
        batch(|| {
            self.lines.update(|x| {
                if let Err(err) = x.clear_inline_completion() {
                    error!("{err:?}");
                }
            });
            self.clear_text_cache();
        });
    }

    pub fn update_inline_completion(&self, delta: &RopeDelta) {
        self.lines.update(|x| {
            if let Err(err) = x.update_inline_completion(delta) {
                error!("{err:?}");
            }
        })
    }

    /// Returns the offsets of the brackets enclosing the given offset.
    /// Uses a language aware algorithm if syntax support is available for the
    /// current language, else falls back to a language unaware algorithm.
    pub fn find_enclosing_brackets(&self, offset: usize) -> Option<(usize, usize)> {
        let rev = self.rev();
        self.lines.with_untracked(|x| {
            if !x.syntax.text.is_empty() && x.syntax.rev == rev {
                x.syntax.find_enclosing_pair(offset)
            } else {
                WordCursor::new(x.buffer().text(), offset).find_enclosing_pair()
            }
        })
    }
}

impl Doc {
    pub fn text(&self) -> Rope {
        self.lines
            .with_untracked(|buffer| buffer.buffer().text().clone())
    }

    // pub fn lines(&self) -> DocLinesManager {
    //     self.lines
    // }
    pub fn rope_text(&self) -> RopeTextVal {
        RopeTextVal::new(self.text())
    }

    pub fn cache_rev(&self) -> RwSignal<u64> {
        self.cache_rev
    }

    // fn visual_line_of_line(&self, line: usize) -> usize {
    //     self.folding_ranges
    //         .with_untracked(|x| x.get_folded_range().visual_line(line))
    // }

    pub fn find_unmatched(&self, offset: usize, previous: bool, ch: char) -> usize {
        self.lines.with_untracked(|x| {
            let syntax = &x.syntax;
            if syntax.layers.is_some() {
                syntax
                    .find_tag(offset, previous, &CharBuffer::from(ch))
                    .unwrap_or(offset)
            } else {
                let text = self.text();
                let mut cursor = WordCursor::new(&text, offset);
                let new_offset = if previous {
                    cursor.previous_unmatched(ch)
                } else {
                    cursor.next_unmatched(ch)
                };

                new_offset.unwrap_or(offset)
            }
        })
    }

    pub fn find_matching_pair(&self, offset: usize) -> usize {
        self.lines.with_untracked(|x| {
            let syntax = &x.syntax;
            if syntax.layers.is_some() {
                syntax.find_matching_pair(offset).unwrap_or(offset)
            } else {
                let text = self.text();
                WordCursor::new(&text, offset)
                    .match_pairs()
                    .unwrap_or(offset)
            }
        })
    }

    pub fn preedit(&self) -> PreeditData {
        self.lines.with_untracked(|x| x.preedit.clone())
    }

    // pub fn run_command(
    //     &self,
    //     ed: &Editor,
    //     cmd: &Command,
    //     count: Option<usize>,
    //     modifiers: Modifiers,
    // ) -> CommandExecuted {
    //     let Some(editor_data) = self.editor_data(ed.id()) else {
    //         return CommandExecuted::No;
    //     };

    //     let cmd = CommandKind::from(cmd.clone());
    //     let cmd = LapceCommand {
    //         kind: cmd,
    //         data: None,
    //     };
    //     editor_data.run_command(&cmd, count, modifiers)
    // }

    // pub fn receive_char(&self, ed: &Editor, c: &str) {
    //     let Some(editor_data) = self.editor_data(ed.id()) else {
    //         return;
    //     };

    //     editor_data.receive_char(c);
    // }

    // pub fn edit(
    //     &self,
    //     iter: &mut dyn Iterator<Item = (Selection, &str)>,
    //     edit_type: EditType,
    // ) {
    //     let delta = self
    //         .lines
    //         .try_update(|buffer| buffer.edit_buffer(iter, edit_type))
    //         .unwrap();
    //     self.apply_deltas(&[delta]);
    // }

    // pub fn editor_id(&self) -> EditorId {
    //     self.editor_id
    // }

    pub fn font_size(&self, _line: usize) -> usize {
        self.lines.with_untracked(|x| x.config.font_size)
    }

    // pub fn font_family(&self, _line: usize) -> String {
    //     self.lines.with_untracked(|x| x.config.font_family.clone())
    // }
    pub fn font_family(
        &self,
        _line: usize,
    ) -> std::borrow::Cow<[floem::text::FamilyOwned]> {
        // TODO: cache this
        Cow::Owned(self.common.config.with_untracked(|config| {
            FamilyOwned::parse_list(&config.editor.font_family).collect()
        }))
    }

    // pub(crate) fn tab_width(&self, ) -> usize {
    //     self.common
    //         .config
    //         .with_untracked(|config| config.editor.tab_width)
    // }
    //
    // pub(crate) fn atomic_soft_tabs(&self, ) -> bool {
    //     self.common
    //         .config
    //         .with_untracked(|config| config.editor.atomic_soft_tabs)
    // }

    // pub fn viewport(&self) -> RwSignal<Rect> {
    //     self.viewport
    // }

    // pub fn editor_style(&self) -> RwSignal<EditorStyle> {
    //     self.editor_style
    // }
}

impl CommonAction for Doc {
    fn exec_motion_mode(
        &self,
        cursor: &mut Cursor,
        motion_mode: MotionMode,
        range: Range<usize>,
        is_vertical: bool,
        register: &mut Register,
    ) {
        let deltas = self.buffer_edit(EditBuffer::ExecuteMotionMode {
            cursor,
            motion_mode,
            range,
            is_vertical,
            register,
        });
        self.apply_deltas(&deltas);
    }

    fn do_edit(
        &self,
        cursor: &mut Cursor,
        cmd: &EditCommand,
        modal: bool,
        register: &mut Register,
        smart_tab: bool,
        screen_lines: Arc<ScreenLines>,
    ) -> bool {
        let deltas = Doc::do_edit(
            self,
            cursor,
            cmd,
            modal,
            register,
            smart_tab,
            screen_lines,
        );
        !deltas.is_empty()
    }
}

// #[derive(Clone)]
// pub struct DocStyling {
//     config: WithLapceConfig,
//     doc: Rc<Doc>,
// }
// impl DocStyling {
// }

impl std::fmt::Debug for Doc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Document {:?}", self.buffer_id))
    }
}
//
// pub fn should_blink(
//     _focus: SignalManager<Focus>,
//     _keyboard_focus: RwSignal<Option<ViewId>>
// ) -> impl Fn() -> bool {
//     move || {
//         let Some(focus) = _focus.try_get_untracked() else {
//             return false;
//         };
//         if matches!(
//             focus,
//             Focus::Workbench
//                 | Focus::Palette
//                 | Focus::Panel(lapce_core::panel::PanelKind::Plugin)
//                 | Focus::Panel(lapce_core::panel::PanelKind::Search)
//                 | Focus::Panel(lapce_core::panel::PanelKind::SourceControl)
//         ) {
//             return true;
//         }
//
//         if _keyboard_focus.get_untracked().is_some() {
//             return true;
//         }
//         false
//     }
// }

// fn extra_styles_for_range(
//     text_layout: &TextLayout,
//     start: usize,
//     end: usize,
//     bg_color: Option<Color>,
//     under_line: Option<Color>,
//     wave_line: Option<Color>,
// ) -> impl Iterator<Item = LineExtraStyle> + '_ {
//     let start_hit = text_layout.hit_position(start);
//     let end_hit = text_layout.hit_position(end);
//
//     // log::info!("start={start_hit:?} end={end_hit:?}");
//     text_layout
//         .layout_runs()
//         .enumerate()
//         .filter_map(move |(current_line, run)| {
//             if current_line < start_hit.line || current_line > end_hit.line {
//                 return None;
//             }
//
//             let x = if current_line == start_hit.line {
//                 start_hit.point.x
//             } else {
//                 run.glyphs.first().map(|g| g.x).unwrap_or(0.0) as f64
//             };
//             let end_x = if current_line == end_hit.line {
//                 end_hit.point.x
//             } else {
//                 run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0) as f64
//             };
//             let width = end_x - x;
//
//             if width == 0.0 {
//                 return None;
//             }
//
//             let height = (run.max_ascent + run.max_descent) as f64;
//             let y = run.line_y as f64 - run.max_ascent as f64;
//
//             Some(LineExtraStyle {
//                 x,
//                 y,
//                 width: Some(width),
//                 height,
//                 bg_color,
//                 under_line,
//                 wave_line,
//             })
//         })
// }
