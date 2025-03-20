use std::collections::VecDeque;

use floem::{
    ext_event::{ExtSendTrigger, create_ext_action, register_ext_trigger},
    prelude::SignalUpdate,
    reactive::{ReadSignal, Scope, with_scope},
};
use parking_lot::Mutex;

pub fn create_signal_from_channel<T: Send + Clone + 'static>(
    cx: Scope, //channel_closed: WriteSignal<bool>
) -> (ReadSignal<Option<T>>, ExtChannel<T>, impl FnOnce(())) {
    let (read, write) = cx.create_signal(None);
    let cx = cx.create_child();
    let trigger = with_scope(cx, ExtSendTrigger::new);
    let data = std::sync::Arc::new(Mutex::new(VecDeque::new()));
    {
        let data = data.clone();
        cx.create_effect(move |_| {
            trigger.track();
            while let Some(value) = data.lock().pop_front() {
                write.set(Some(value));
            }
        });
    }
    let send = create_ext_action(cx, move |_| {
        // channel_closed.set(true);
        cx.dispose();
    });

    (read, ExtChannel { trigger, data }, send)
}

#[derive(Clone)]
pub struct ExtChannel<T: Send + Clone + 'static> {
    trigger: ExtSendTrigger,
    data:    std::sync::Arc<Mutex<VecDeque<T>>>,
}

impl<T: Send + Clone + 'static> ExtChannel<T> {
    pub fn send(&mut self, event: T) {
        self.data.lock().push_back(event);
        register_ext_trigger(self.trigger);
    }
}
