pub use head::*;
use lapce_core::icon::LapceIcons;

pub mod call_back;
mod head;

pub trait TabHead: 'static {
    fn icon(&self) -> &'static str {
        LapceIcons::FILE
    }
}
