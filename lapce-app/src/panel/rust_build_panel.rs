use cozy_floem::views::tree_with_panel::tree_with_panel;
use floem::prelude::{Decorators};
use floem::View;
use crate::panel::position::PanelContainerPosition;
use crate::window_workspace::WindowWorkspaceData;

pub fn build_panel(
    window_tab_data: WindowWorkspaceData,
    _position: PanelContainerPosition,
) -> impl View {
    let data = window_tab_data.build_data;
    tree_with_panel(data).style(|x| x.size_full())
}

