use std::{borrow::Cow, ops::Range, path::PathBuf, str::FromStr};

use doc::lines::buffer::{
    rope_text::{RopeText, RopeTextRef},
    Buffer,
};
use doc::lines::selection::Selection;
use doc::lines::RopeTextPosition;
use floem::reactive::{batch, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith};
use log::error;
use lsp_types::InsertTextFormat;

use crate::{config::LapceConfig, doc::Doc, editor::EditorData, snippet::Snippet};

// TODO: we could integrate completion lens with this, so it is considered at the same time

/// Redefinition of lsp types inline completion item with offset range
#[derive(Debug, Clone)]
pub struct InlineCompletionItem {
    /// The text to replace the range with.
    pub insert_text: String,
    /// Text used to decide if this inline completion should be shown.
    pub filter_text: Option<String>,
    /// The range (of offsets) to replace  
    pub range: Option<Range<usize>>,
    pub command: Option<lsp_types::Command>,
    pub insert_text_format: Option<InsertTextFormat>,
}
impl InlineCompletionItem {
    pub fn from_lsp(buffer: &Buffer, item: lsp_types::InlineCompletionItem) -> Self {
        let range = item.range.and_then(|r| {
            let start = match buffer.offset_of_position(&r.start) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return None;
                }
            };
            let end = match buffer.offset_of_position(&r.end) {
                Ok(rs) => rs,
                Err(err) => {
                    error!("{err:?}");
                    return None;
                }
            };
            Some(start..end)
        });
        Self {
            insert_text: item.insert_text,
            filter_text: item.filter_text,
            range,
            command: item.command,
            insert_text_format: item.insert_text_format,
        }
    }

    pub fn apply(
        &self,
        editor: &EditorData,
        start_offset: usize,
    ) -> anyhow::Result<()> {
        let text_format = self
            .insert_text_format
            .unwrap_or(InsertTextFormat::PLAIN_TEXT);

        let selection = if let Some(range) = &self.range {
            Selection::region(range.start, range.end)
        } else {
            Selection::caret(start_offset)
        };

        match text_format {
            InsertTextFormat::PLAIN_TEXT => editor.do_edit(
                &selection,
                &[(selection.clone(), self.insert_text.as_str())],
                false,
            ),
            InsertTextFormat::SNIPPET => {
                let snippet = Snippet::from_str(&self.insert_text)?;
                let text = snippet.text();
                let additional_edit = vec![(selection.clone(), text.as_str())];

                editor.completion_apply_snippet(
                    snippet,
                    &selection,
                    additional_edit,
                    start_offset,
                )?;
            }
            _ => {
                // We don't know how to support this text format
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionStatus {
    /// The inline completion is not active.
    Inactive,
    /// The inline completion is active and is waiting for the server to respond.
    Started,
    /// The inline completion is active and has received a response from the server.
    Active,
}

#[derive(Clone)]
pub struct InlineCompletionData {
    pub status: InlineCompletionStatus,
    /// The active inline completion index in the list of completions.
    pub active: RwSignal<usize>,
    pub items: im::Vector<InlineCompletionItem>,
    pub start_offset: usize,
    pub path: PathBuf,
}
impl InlineCompletionData {
    pub fn new(cx: Scope) -> Self {
        Self {
            status: InlineCompletionStatus::Inactive,
            active: cx.create_rw_signal(0),
            items: im::vector![],
            start_offset: 0,
            path: PathBuf::new(),
        }
    }

    pub fn current_item(&self) -> Option<&InlineCompletionItem> {
        let active = self.active.get_untracked();
        self.items.get(active)
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            let next_index = (self.active.get_untracked() + 1) % self.items.len();
            self.active.set(next_index);
        }
    }

    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            let prev_index = if self.active.get_untracked() == 0 {
                self.items.len() - 1
            } else {
                self.active.get_untracked() - 1
            };
            self.active.set(prev_index);
        }
    }

    pub fn cancel(&mut self) {
        if self.status == InlineCompletionStatus::Inactive {
            return;
        }

        self.items.clear();
        self.status = InlineCompletionStatus::Inactive;
    }

    /// Set the items for the inline completion.  
    /// Sets `active` to `0` and `status` to `InlineCompletionStatus::Active`.
    pub fn set_items(
        &mut self,
        items: im::Vector<InlineCompletionItem>,
        start_offset: usize,
        path: PathBuf,
    ) {
        batch(|| {
            self.items = items;
            self.active.set(0);
            self.status = InlineCompletionStatus::Active;
            self.start_offset = start_offset;
            self.path = path;
        });
    }

    pub fn update_doc(&self, doc: &Doc, offset: usize) {
        if self.status != InlineCompletionStatus::Active {
            doc.clear_inline_completion();
            return;
        }

        if self.items.is_empty() {
            doc.clear_inline_completion();
            return;
        }

        let active = self.active.get_untracked();
        let active = if active >= self.items.len() {
            self.active.set(0);
            0
        } else {
            active
        };

        let item = &self.items[active];
        let text = item.insert_text.clone();

        // TODO: is range really meant to be used for this?
        let offset = item.range.as_ref().map(|r| r.start).unwrap_or(offset);
        let (line, col) = match doc
            .lines
            .with_untracked(|x| x.buffer().offset_to_line_col(offset))
        {
            Ok(rs) => rs,
            Err(err) => {
                error!("{err:?}");
                return;
            }
        };
        doc.set_inline_completion(text, line, col);
    }

    pub fn update_inline_completion(
        &self,
        config: &LapceConfig,
        doc: &Doc,
        cursor_offset: usize,
    ) {
        if !config.editor.enable_inline_completion {
            doc.clear_inline_completion();
            return;
        }

        let text = doc.lines.with_untracked(|x| x.buffer().text().clone());
        let text = RopeTextRef::new(&text);
        let Some(item) = self.current_item() else {
            // TODO(minor): should we cancel completion
            return;
        };

        let completion = doc.lines.with_untracked(|cur| {
            let cur = cur.inline_completion.as_ref().map(|x| x.0.as_str());
            inline_completion_text(text, self.start_offset, cursor_offset, item, cur)
        });

        match completion {
            ICompletionRes::Hide => {
                doc.clear_inline_completion();
            }
            ICompletionRes::Unchanged => {}
            ICompletionRes::Set(new, shift) => {
                let offset = self.start_offset + shift;
                let (line, col) = match text.offset_to_line_col(offset) {
                    Ok(rs) => rs,
                    Err(err) => {
                        error!("{err:?}");
                        return;
                    }
                };
                doc.set_inline_completion(new, line, col);
            }
        }
    }
}

enum ICompletionRes {
    Hide,
    Unchanged,
    Set(String, usize),
}

/// Get the text of the inline completion item  
fn inline_completion_text(
    rope_text: impl RopeText,
    start_offset: usize,
    cursor_offset: usize,
    item: &InlineCompletionItem,
    current_completion: Option<&str>,
) -> ICompletionRes {
    let text_format = item
        .insert_text_format
        .unwrap_or(InsertTextFormat::PLAIN_TEXT);

    // TODO: is this check correct? I mostly copied it from completion lens
    let cursor_prev_offset = rope_text.prev_code_boundary(cursor_offset);
    if let Some(range) = &item.range {
        let edit_start = range.start;

        // If the start of the edit isn't where the cursor currently is, and is not at the start of
        // the inline completion, then we ignore it.
        if cursor_prev_offset != edit_start && start_offset != edit_start {
            return ICompletionRes::Hide;
        }
    }

    let text = match text_format {
        InsertTextFormat::PLAIN_TEXT => Cow::Borrowed(&item.insert_text),
        InsertTextFormat::SNIPPET => {
            let Ok(snippet) = Snippet::from_str(&item.insert_text) else {
                return ICompletionRes::Hide;
            };
            let text = snippet.text();

            Cow::Owned(text)
        }
        _ => {
            // We don't know how to support this text format
            return ICompletionRes::Hide;
        }
    };

    let range = start_offset..match rope_text.offset_line_end(start_offset, true) {
        Ok(rs) => rs,
        Err(err) => {
            error!("{err:?}");
            return ICompletionRes::Unchanged;
        }
    };
    let prefix = rope_text.slice_to_cow(range);
    // We strip the prefix of the current input from the label.
    // So that, for example `p` with a completion of `println` will show `rintln`.
    let Some(text) = text.strip_prefix(prefix.as_ref()) else {
        return ICompletionRes::Hide;
    };

    if Some(text) == current_completion {
        ICompletionRes::Unchanged
    } else {
        ICompletionRes::Set(text.to_string(), prefix.len())
    }
}
