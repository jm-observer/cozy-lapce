use std::{path::PathBuf, rc::Rc};

use doc::lines::{
    command::FocusCommand, editor_command::CommandExecuted, mode::Mode
};
use floem::{
    ext_event::create_ext_action,
    keyboard::Modifiers,
    peniko::kurbo::Rect,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, batch}
};
use lapce_rpc::proxy::ProxyResponse;
use lsp_types::Position;

use crate::{
    command::{CommandKind, InternalCommand, LapceCommand},
    keypress::{KeyPressFocus, condition::Condition},
    window_workspace::{CommonData, Focus}
};

#[derive(Clone, Debug)]
pub struct RenameData {
    pub active:      RwSignal<bool>,
    pub name_str:    RwSignal<String>,
    pub start:       RwSignal<usize>,
    pub position:    RwSignal<Position>,
    pub path:        RwSignal<PathBuf>,
    pub layout_rect: RwSignal<Rect>,
    pub common:      Rc<CommonData>
}

impl KeyPressFocus for RenameData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(condition, Condition::RenameFocus | Condition::ModalFocus)
    }

    fn run_command(
        &self,
        _command: &LapceCommand,
        _count: Option<usize>,
        _mods: Modifiers
    ) -> CommandExecuted {
        if let CommandKind::Focus(cmd) = &_command.kind {
            self.run_focus_command(cmd);
        }
        CommandExecuted::Yes
    }

    fn receive_char(&self, _c: &str) {}
}

impl RenameData {
    pub fn new(cx: Scope, common: Rc<CommonData>) -> Self {
        let active = cx.create_rw_signal(false);
        let start = cx.create_rw_signal(0);
        let position = cx.create_rw_signal(Position::default());
        let layout_rect = cx.create_rw_signal(Rect::ZERO);
        let path = cx.create_rw_signal(PathBuf::new());
        let name_str = cx.create_rw_signal(String::new());

        Self {
            active,
            start,
            position,
            layout_rect,
            path,
            common,
            name_str
        }
    }

    pub fn start(
        &self,
        path: PathBuf,
        placeholder: String,
        start: usize,
        position: Position
    ) {
        batch(|| {
            self.name_str.set(placeholder);
            self.path.set(path);
            self.start.set(start);
            self.position.set(position);
            self.active.set(true);
            self.common.focus.set(Focus::Rename);
        });
    }

    fn run_focus_command(&self, cmd: &FocusCommand) -> CommandExecuted {
        match cmd {
            FocusCommand::ModalClose => {
                self.cancel();
            },
            FocusCommand::ConfirmRename => {
                self.confirm();
            },
            _ => return CommandExecuted::No
        }
        CommandExecuted::Yes
    }

    fn cancel(&self) {
        self.active.set(false);
        if let Focus::Rename = self.common.focus.get_untracked() {
            self.common.focus.set(Focus::Workbench);
        }
    }

    fn confirm(&self) {
        let new_name = self.name_str.get_untracked();
        log::info!("confirm {new_name}");
        let new_name = new_name.trim();
        if !new_name.is_empty() {
            let path = self.path.get_untracked();
            let position = self.position.get_untracked();
            let internal_command = self.common.internal_command;
            let send = create_ext_action(self.common.scope, move |result| {
                if let Ok(ProxyResponse::Rename { edit }) = result {
                    internal_command
                        .send(InternalCommand::ApplyWorkspaceEdit { edit });
                }
            });
            self.common.proxy.rename(
                path,
                position,
                new_name.to_string(),
                move |(_, result)| {
                    send(result);
                }
            );
        }
        self.cancel();
    }
}
