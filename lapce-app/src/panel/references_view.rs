use std::rc::Rc;

use floem::views::stack;
use floem::{views::Decorators, View};

use crate::common::common_tab_header;
use crate::panel::position::PanelContainerPosition;
use crate::{
    panel::implementation_view::common_reference_panel, window_tab::WindowTabData,
};

pub fn references_panel(
    window_tab_data: Rc<WindowTabData>,
    _position: PanelContainerPosition,
) -> impl View {
    stack((
        common_tab_header(
            window_tab_data.clone(),
            window_tab_data.main_split.references.clone(),
        ),
        common_reference_panel(window_tab_data.clone(), _position, move || {
            window_tab_data
                .main_split
                .references
                .get_active_content()
                .unwrap_or_default()
        })
        .debug_name("references panel"),
    ))
    .style(|x| x.flex_col().width_full().height_full())
}
