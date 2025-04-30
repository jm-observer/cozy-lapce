use floem::{ext_event::create_ext_action, reactive::Scope};
use lapce_rpc::{
    RpcError,
    proxy::{FileAndLine, ProxyCallback, ProxyResponse},
};
use log::error;
use lsp_types::Position;

use crate::{
    command::InternalCommand,
    editor::location::{EditorLocation, EditorPosition},
    listener::Listener,
};

pub fn find_file_call_back(
    scope: Scope,
    internal_command: Listener<InternalCommand>,
) -> impl ProxyCallback + 'static {
    create_ext_action(
        scope,
        move |(_id, response): (u64, Result<ProxyResponse, RpcError>)| match response
        {
            Ok(response) => {
                if let ProxyResponse::FindFileFromLogResponse { rs } = response {
                    match rs {
                        lapce_rpc::RpcResult::Err(err) => internal_command.send(
                            InternalCommand::ShowStatusMessage {
                                message: err.to_string(),
                            },
                        ),
                        lapce_rpc::RpcResult::Ok(FileAndLine { file, line }) => {
                            internal_command.send(InternalCommand::JumpToLocation {
                                location: EditorLocation {
                                    path:               file,
                                    position:           Some(
                                        EditorPosition::Position(Position::new(
                                            line.saturating_sub(1),
                                            0,
                                        )),
                                    ),
                                    scroll_offset:      None,
                                    ignore_unconfirmed: false,
                                    same_editor_tab:    false,
                                },
                            })
                        },
                    }
                }
            },
            Err(err) => {
                internal_command.send(InternalCommand::ShowStatusMessage {
                    message: err.to_string(),
                });
                error!("{err:?}");
            },
        },
    )
}
