use std::collections::HashMap;
use std::sync::Arc;
use crossbeam_channel::{Receiver};
use parking_lot::Mutex;
use lapce_proxy::plugin::wasi::find_all_volts;
use lapce_rpc::{RequestId};
use crate::local_task::{LocalRequest, LocalResponse, LocalResponseHandler, LocalRpc};
use anyhow::Result;
use lapce_core::directory::Directory;

#[derive(Clone)]
pub struct LocalTaskHandler {
    pub directory: Directory,
    pub(crate) rx:      Receiver<LocalRpc>,
    pub(crate) pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>
}

impl LocalTaskHandler {

    pub async fn handle(&self)
    {
        use crate::local_task::LocalRpc::*;
        for msg in &self.rx {
            match msg {
                Request {id, request} => {
                    match request {
                        LocalRequest::FindAllVolts { extra_plugin_paths } => {
                            let volts = find_all_volts(&extra_plugin_paths, &self.directory.plugins_directory).await;
                            self.handle_response(id, Ok(LocalResponse::FindAllVolts {volts}));
                        }
                    }
                },
                Notification{notification} => {
                },
                Shutdown => {
                    return;
                }
            }
        }
    }

    pub fn handle_response(
        &self,
        id: RequestId,
        result: Result<LocalResponse>
    ) {
        let handler = { self.pending.lock().remove(&id) };
        if let Some(handler) = handler {
            handler.invoke(id, result);
        }
    }
}