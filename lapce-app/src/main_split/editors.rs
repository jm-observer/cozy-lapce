use std::{path::Path, rc::Rc};

use doc::EditorViewKind;
use floem::{
    prelude::{RwSignal, SignalUpdate, SignalWith},
    reactive::Scope,
};
use lapce_core::id::{DiffEditorId, EditorId, EditorTabManageId};
use log::warn;

use crate::{doc::Doc, editor::EditorData, window_workspace::CommonData};

/// All the editors in a main split

#[derive(Clone, Copy)]
pub struct Editors(RwSignal<im::HashMap<EditorId, EditorData>>);

impl Editors {
    pub fn new(cx: Scope) -> Self {
        Self(cx.create_rw_signal(im::HashMap::new()))
    }

    /// Add an editor to the editors.  
    /// Returns the id of the editor.
    pub fn insert(&self, editor: EditorData) -> EditorId {
        let id = editor.id();
        self.0.update(|editors| {
            if editors.insert(id, editor).is_some() {
                warn!("Inserted EditorId that already exists");
            }
        });

        id
    }

    pub fn insert_with_id(&self, id: EditorId, editor: EditorData) {
        self.0.update(|editors| {
            editors.insert(id, editor);
        });
    }

    pub fn new_local(
        &self,
        cx: Scope,
        common: Rc<CommonData>,
        name: Option<String>,
    ) -> EditorId {
        let editor = EditorData::new_local(cx, common, name);

        self.insert(editor)
    }

    /// Equivalent to [`Self::new_local`], but immediately gets the created
    /// editor.
    pub fn make_local(&self, cx: Scope, common: Rc<CommonData>) -> EditorData {
        let id = self.new_local(cx, common, None);
        self.editor_untracked(id).unwrap()
    }

    /// Equivalent to [`Self::new_local`], but immediately gets the created
    /// editor.
    pub fn make_local_with_name(
        &self,
        cx: Scope,
        common: Rc<CommonData>,
        name: String,
    ) -> EditorData {
        let id = self.new_local(cx, common, Some(name));
        self.editor_untracked(id).unwrap()
    }

    pub fn new_from_doc(
        &self,
        cx: Scope,
        doc: Rc<Doc>,
        editor_tab_id: Option<EditorTabManageId>,
        diff_editor_id: Option<(EditorTabManageId, DiffEditorId)>,
        common: Rc<CommonData>,
        view_kind: EditorViewKind,
    ) -> EditorId {
        let editor = EditorData::new_doc(
            cx,
            doc,
            editor_tab_id,
            diff_editor_id,
            common,
            view_kind,
        );

        self.insert(editor)
    }

    /// Equivalent to [`Self::new_editor_doc`], but immediately gets the created
    /// editor.
    pub fn make_from_doc(
        &self,
        cx: Scope,
        doc: Rc<Doc>,
        editor_tab_id: Option<EditorTabManageId>,
        diff_editor_id: Option<(EditorTabManageId, DiffEditorId)>,
        common: Rc<CommonData>,
        view_kind: EditorViewKind,
    ) -> EditorData {
        let id = self.new_from_doc(
            cx,
            doc,
            editor_tab_id,
            diff_editor_id,
            common,
            view_kind,
        );
        self.editor_untracked(id).unwrap()
    }

    /// Copy an existing editor which is inserted into [`Editors`]
    pub fn copy(
        &self,
        editor_id: EditorId,
        cx: Scope,
        editor_tab_id: Option<EditorTabManageId>,
        diff_editor_id: Option<(EditorTabManageId, DiffEditorId)>,
    ) -> Option<EditorId> {
        let editor = self.editor_untracked(editor_id)?;
        let new_editor = editor.copy(cx, editor_tab_id, diff_editor_id);

        Some(self.insert(new_editor))
    }

    pub fn make_copy(
        &self,
        editor_id: EditorId,
        cx: Scope,
        editor_tab_id: Option<EditorTabManageId>,
        diff_editor_id: Option<(EditorTabManageId, DiffEditorId)>,
    ) -> Option<EditorData> {
        let editor_id = self.copy(editor_id, cx, editor_tab_id, diff_editor_id)?;
        self.editor_untracked(editor_id)
    }

    pub fn remove(&self, id: EditorId) -> Option<EditorData> {
        self.0.try_update(|editors| editors.remove(&id)).unwrap()
    }

    pub fn get_editor_id_by_path(&self, path: &Path) -> Option<EditorId> {
        self.0.with_untracked(|x| {
            for (id, data) in x {
                if data.doc().content.with_untracked(|x| {
                    if let Some(doc_path) = x.path() {
                        doc_path == path
                    } else {
                        false
                    }
                }) {
                    return Some(*id);
                }
            }
            None
        })
    }

    pub fn contains_untracked(&self, id: EditorId) -> bool {
        self.0.with_untracked(|editors| editors.contains_key(&id))
    }

    /// Get the editor (tracking the signal)
    pub fn editor(&self, id: EditorId) -> Option<EditorData> {
        self.0.with(|editors| editors.get(&id).cloned())
    }

    /// Get the editor (not tracking the signal)
    pub fn editor_untracked(&self, id: EditorId) -> Option<EditorData> {
        self.0.with_untracked(|editors| editors.get(&id).cloned())
    }

    pub fn with_editors<O>(
        &self,
        f: impl FnOnce(&im::HashMap<EditorId, EditorData>) -> O,
    ) -> O {
        self.0.with(f)
    }

    pub fn with_editors_untracked<O>(
        &self,
        f: impl FnOnce(&im::HashMap<EditorId, EditorData>) -> O,
    ) -> O {
        self.0.with_untracked(f)
    }
}
