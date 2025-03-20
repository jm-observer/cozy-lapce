use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crossbeam_channel::Sender;
use parking_lot::Mutex;

use crate::local_task::{
    LocalCallback, LocalNotification, LocalRequest, LocalResponseHandler, LocalRpc,
};

#[derive(Clone)]
pub struct LocalTaskRequester {
    tx:      Sender<LocalRpc>,
    id:      Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>,
}

impl LocalTaskRequester {
    pub(super) fn new(
        tx: Sender<LocalRpc>,
        pending: Arc<Mutex<HashMap<u64, LocalResponseHandler>>>,
    ) -> Self {
        Self {
            tx,
            pending,
            id: Arc::new(AtomicU64::new(0)),
        }
    }

    fn request_common(
        &self,
        request: LocalRequest,
        rh: LocalResponseHandler,
    ) -> u64 {
        let id = self.id.fetch_add(1, Ordering::Relaxed);

        self.pending.lock().insert(id, rh);

        if let Err(err) = self.tx.send(LocalRpc::Request { id, request }) {
            log::error!("{:?}", err);
        }
        id
    }

    // pub fn request(&self, request: LocalRequest) -> Result<LocalResponse,
    // RpcError> {     let (tx, rx) = crossbeam_channel::bounded(1);
    //     self.request_common(request, LocalResponseHandler::Chan(tx));
    //     rx.recv()
    //         .map_err(|err| RpcError {
    //             code: 0,
    //             message: "io error".to_string()
    //         })?
    //         .1
    // }

    pub fn request_async(
        &self,
        request: LocalRequest,
        f: impl LocalCallback + 'static,
    ) -> u64 {
        self.request_common(request, LocalResponseHandler::Callback(Box::new(f)))
    }

    pub fn notification(&self, notification: LocalNotification) {
        if let Err(err) = self.tx.send(LocalRpc::Notification { notification }) {
            log::error!("{:?}", err);
        }
    }
}
