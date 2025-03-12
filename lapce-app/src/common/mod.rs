pub use head::*;
use lapce_core::icon::LapceIcons;

mod head;

pub trait TabHead: 'static {
    fn icon(&self) -> &'static str {
        LapceIcons::FILE
    }
}
