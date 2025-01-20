use std::rc::Rc;
use lapce_xi_rope::Rope;
use serde::{Deserialize, Serialize};
use lapce_rpc::buffer::BufferId;
use lapce_rpc::plugin::VoltID;
use lapce_rpc::proxy::ProxyResponse;
use crate::doc::DocContent;

#[derive(Clone, Serialize, Deserialize)]
pub struct EditorTabInfo {
    pub active: usize,
    pub is_focus: bool,
    pub children: Vec<EditorTabChildInfo>,
}

impl EditorTabInfo {
    // pub fn to_data(
    //     &self,
    //     data: MainSplitData,
    //     split: SplitId,
    // ) -> RwSignal<EditorTabManageData> {
    //     let editor_tab_id = EditorTabManageId::next();
    //     let editor_tab_data = {
    //         let cx = data.scope.create_child();
    //         let editor_tab_data = EditorTabManageData {
    //             scope: cx,
    //             editor_tab_manage_id: editor_tab_id,
    //             split,
    //             active: self.active,
    //             children: self
    //                 .children
    //                 .iter()
    //                 .map(|child| {
    //                     EditorTabChildSimple::new(
    //                         cx.create_rw_signal(0),
    //                         cx.create_rw_signal(Rect::ZERO),
    //                         child.to_data(data.clone(), editor_tab_id),
    //                     )
    //                 })
    //                 .collect(),
    //             layout_rect: Rect::ZERO,
    //             window_origin: Point::ZERO,
    //             locations: cx.create_rw_signal(im::Vector::new()),
    //             current_location: cx.create_rw_signal(0),
    //         };
    //         cx.create_rw_signal(editor_tab_data)
    //     };
    //     if self.is_focus {
    //         data.active_editor_tab.set(Some(editor_tab_id));
    //     }
    //     data.editor_tabs.update(|editor_tabs| {
    //         editor_tabs.insert(editor_tab_id, editor_tab_data);
    //     });
    //     editor_tab_data
    // }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum EditorTabChildInfo {
    Editor(EditorInfo),
    DiffEditor(DiffEditorInfo),
    Settings,
    ThemeColorSettings,
    Keymap,
    Volt(VoltID),
}

impl EditorTabChildInfo {
    // pub fn to_data(
    //     &self,
    //     data: MainSplitData,
    //     editor_tab_id: EditorTabManageId,
    // ) -> EditorTabChildId {
    //     match &self {
    //         EditorTabChildInfo::Editor(editor_info) => {
    //             let editor_id = editor_info.to_data(data, editor_tab_id);
    //             EditorTabChildId::Editor(editor_id)
    //         },
    //         EditorTabChildInfo::DiffEditor(diff_editor_info) => {
    //             let diff_editor_data = diff_editor_info.to_data(data, editor_tab_id);
    //             EditorTabChildId::DiffEditor(diff_editor_data.id)
    //         },
    //         EditorTabChildInfo::Settings => {
    //             EditorTabChildId::Settings(SettingsId::next())
    //         },
    //         EditorTabChildInfo::ThemeColorSettings => {
    //             EditorTabChildId::ThemeColorSettings(ThemeColorSettingsId::next())
    //         },
    //         EditorTabChildInfo::Keymap => EditorTabChildId::Keymap(KeymapId::next()),
    //         EditorTabChildInfo::Volt(id) => {
    //             EditorTabChildId::Volt(VoltViewId::next(), id.to_owned())
    //         },
    //     }
    // }
}



#[derive(Clone, Serialize, Deserialize)]
pub struct EditorInfo {
    pub content: DocContent,
    pub unsaved: Option<String>,
    pub offset: usize,
    pub scroll_offset: (f64, f64),
}

impl EditorInfo {
    // pub fn to_data(
    //     &self,
    //     data: MainSplitData,
    //     editor_tab_id: EditorTabManageId,
    // ) -> EditorId {
    //     let editors = &data.editors;
    //     let common = data.common.clone();
    //     match &self.content {
    //         DocContent::File { path, .. } => {
    //             let (doc, new_doc) =
    //                 data.get_doc(path.clone(), self.unsaved.clone(), true);
    //             let editor = editors.make_from_doc(
    //                 data.scope,
    //                 doc,
    //                 Some(editor_tab_id),
    //                 None,
    //                 None,
    //                 common,
    //             );
    //             editor.go_to_location(
    //                 EditorLocation {
    //                     path: path.clone(),
    //                     position: Some(EditorPosition::Offset(self.offset)),
    //                     scroll_offset: Some(Vec2::new(
    //                         self.scroll_offset.0,
    //                         self.scroll_offset.1,
    //                     )),
    //                     ignore_unconfirmed: false,
    //                     same_editor_tab: false,
    //                 },
    //                 new_doc,
    //                 None,
    //             );
    //
    //             editor.id()
    //         }
    //         DocContent::Local => editors.new_local(data.scope, common, None),
    //         DocContent::History(_) => editors.new_local(data.scope, common, None),
    //         DocContent::Scratch { name, .. } => {
    //             let doc = data
    //                 .scratch_docs
    //                 .try_update(|scratch_docs| {
    //                     if let Some(doc) = scratch_docs.get(name) {
    //                         return doc.clone();
    //                     }
    //                     let content = DocContent::Scratch {
    //                         id: BufferId::next(),
    //                         name: name.to_string(),
    //                     };
    //                     let doc = Doc::new_content(
    //                         data.scope,
    //                         content,
    //                         data.editors,
    //                         data.common.clone(),
    //                         None,
    //                     );
    //                     let doc = Rc::new(doc);
    //                     if let Some(unsaved) = &self.unsaved {
    //                         doc.reload(Rope::from(unsaved), false);
    //                     }
    //                     scratch_docs.insert(name.to_string(), doc.clone());
    //                     doc
    //                 })
    //                 .unwrap();
    //
    //             editors.new_from_doc(
    //                 data.scope,
    //                 doc,
    //                 Some(editor_tab_id),
    //                 None,
    //                 None,
    //                 common,
    //             )
    //         }
    //     }
    // }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DiffEditorInfo {
    pub left_content: DocContent,
    pub right_content: DocContent,
}

impl DiffEditorInfo {
    // pub fn to_data(
    //     &self,
    //     data: MainSplitData,
    //     editor_tab_id: EditorTabManageId,
    // ) -> DiffEditorData {
    //     let cx = data.scope.create_child();
    //
    //     let diff_editor_id = DiffEditorId::next();
    //
    //     let new_doc = {
    //         let data = data.clone();
    //         let common = data.common.clone();
    //         move |content: &DocContent| match content {
    //             DocContent::File { path, .. } => {
    //                 let (doc, _) = data.get_doc(path.clone(), None, false);
    //                 doc
    //             },
    //             DocContent::Local => {
    //                 Rc::new(Doc::new_local(cx, data.editors, common.clone(), None))
    //             },
    //             DocContent::History(history) => {
    //                 let doc = Doc::new_history(
    //                     cx,
    //                     content.clone(),
    //                     data.editors,
    //                     common.clone(),
    //                 );
    //                 let doc = Rc::new(doc);
    //
    //                 {
    //                     let doc = doc.clone();
    //                     let send = create_ext_action(cx, move |result| {
    //                         if let Ok(ProxyResponse::BufferHeadResponse {
    //                                       content,
    //                                       ..
    //                                   }) = result
    //                         {
    //                             doc.init_content(Rope::from(content));
    //                         }
    //                     });
    //                     common.proxy.get_buffer_head(
    //                         history.path.clone(),
    //                         move |(_, result)| {
    //                             send(result);
    //                         },
    //                     );
    //                 }
    //
    //                 doc
    //             },
    //             DocContent::Scratch { name, .. } => {
    //                 let doc_content = DocContent::Scratch {
    //                     id: BufferId::next(),
    //                     name: name.to_string(),
    //                 };
    //                 let doc = Doc::new_content(
    //                     cx,
    //                     doc_content,
    //                     data.editors,
    //                     common.clone(),
    //                     None,
    //                 );
    //                 let doc = Rc::new(doc);
    //                 data.scratch_docs.update(|scratch_docs| {
    //                     scratch_docs.insert(name.to_string(), doc.clone());
    //                 });
    //                 doc
    //             },
    //         }
    //     };
    //
    //     let left_doc = new_doc(&self.left_content);
    //     let right_doc = new_doc(&self.right_content);
    //
    //     let diff_editor_data = DiffEditorData::new(
    //         cx,
    //         diff_editor_id,
    //         editor_tab_id,
    //         left_doc,
    //         right_doc,
    //         data.editors,
    //         data.common.clone(),
    //     );
    //
    //     data.diff_editors.update(|diff_editors| {
    //         diff_editors.insert(diff_editor_id, diff_editor_data.clone());
    //     });
    //
    //     diff_editor_data
    // }
}