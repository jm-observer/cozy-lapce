#[derive(strum_macros::AsRefStr, PartialEq, Eq, Debug)]
pub enum ReleaseType {
    Debug,
    Stable,
    Nightly,
}

include!(concat!(env!("OUT_DIR"), "/meta.rs"));
