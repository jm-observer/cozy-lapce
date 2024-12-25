use lapce_core::directory::Directory;
use log::{error, trace};

use crate::log::*;

pub(super) fn panic_hook() {
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("main");
        let backtrace = backtrace::Backtrace::new();

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else {
            "<unknown>"
        };

        match info.location() {
            Some(loc) => {
                error!(
                    target: "lapce_app::panic_hook",

                    "thread {thread} panicked at {} | file://./{}:{}:{}\n{:?}",
                    payload,
                    loc.file(), loc.line(), loc.column(),
                    backtrace,
                );
            }
            None => {
                error!(
                    target: "lapce_app::panic_hook",

                    "thread {thread} panicked at {}\n{:?}",
                    payload,
                    backtrace,
                );
            }
        }

        #[cfg(windows)]
        error_modal("Error", &info.to_string());
    }))
}

#[cfg(windows)]
pub(super) fn error_modal(title: &str, msg: &str) -> i32 {
    use std::{ffi::OsStr, iter::once, mem, os::windows::prelude::OsStrExt};

    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_SYSTEMMODAL,
    };

    let result: i32;

    let title = OsStr::new(title)
        .encode_wide()
        .chain(once(0u16))
        .collect::<Vec<u16>>();
    let msg = OsStr::new(msg)
        .encode_wide()
        .chain(once(0u16))
        .collect::<Vec<u16>>();
    unsafe {
        result = MessageBoxW(
            mem::zeroed(),
            msg.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR | MB_SYSTEMMODAL,
        );
    }

    result
}
