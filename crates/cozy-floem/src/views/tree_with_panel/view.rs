use floem::{
    View,
    prelude::{
        Decorators, SignalGet, SignalUpdate, container, scroll, stack, virtual_stack,
    },
    style::{AlignItems, FlexDirection},
    views::static_label,
};

use crate::views::{
    drag_line::x_drag_line,
    panel::panel,
    svg_from_fn,
    tree_with_panel::data::{Level, TreePanelData},
};

pub fn tree_with_panel(data: TreePanelData) -> impl View {
    let left_width = data.left_width;
    let doc = data.doc;
    stack((
        view_tree(data).style(move |x| {
            let width = left_width.get();
            x.width(width)
                .height_full()
                .border_left(1.)
                .border_top(1.)
                .border_bottom(1.)
                .border_right(1.0)
        }),
        x_drag_line(left_width)
            .style(move |s| s.width(6.0).height_full().margin_left(-6.0)),
        panel(doc).style(|x| x.flex_grow(1.).height_full()),
    ))
}
fn view_tree(data: TreePanelData) -> impl View {
    let node = data.node;
    scroll(
        virtual_stack(
            // VirtualDirection::Vertical,
            // VirtualItemSize::Fixed(Box::new(move || 20.0)),
            move || node.get(),
            move |(_index, _, _data)| _data.display_id.clone(),
            move |(_, retract, rw_data)| {
                let id = rw_data.display_id.clone();
                let level = rw_data.level;
                let level_svg_color = rw_data.track_level_svg_color();

                let click_data = rw_data.open;
                stack(
                    (container(svg_from_fn(move || if click_data.get() {
                            r#"<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M7.976 10.072l4.357-4.357.62.618L8.284 11h-.618L3 6.333l.619-.618 4.357 4.357z"/></svg>"#
                    } else {
                        r#"<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M10.072 8.024L5.715 3.667l.618-.62L11 7.716v.618L6.333 13l-.618-.619 4.357-4.357z"/></svg>"#
                    }.to_string()).style(move |s| {
                        let size = 13.0;
                            s.size(size, size)
                    })).on_click_stop(move |_ | {
                        click_data.update(move |x| {
                            *x = !*x
                        });
                    }),
                    container(svg_from_fn(move || match level.get() {
                        Level::None => {
                            // empty.svg
                            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16"></svg>"#
                        }
                        Level::Warn | Level::Error => {
                            // warning.svg
                            r#"<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><path fill-rule="evenodd" clip-rule="evenodd" d="M7.56 1h.88l6.54 12.26-.44.74H1.44L1 13.26 7.56 1zM8 2.28L2.28 13H13.7L8 2.28zM8.625 12v-1h-1.25v1h1.25zm-1.25-2V6h1.25v4h-1.25z"/></svg>"#
                        }
                    }.to_string()).style(move |s| {
                        let size = 13.0;
                        if let Some(color) = level_svg_color {
                            s.size(size, size).color(color)
                        } else {
                            s.size(size, size)
                        }
                    })),
                static_label(rw_data.display_id.head()).style(move |x| x.height(23.).font_size(13.).align_self(AlignItems::Start))
                    .on_click_stop(move |_ | {
                        let value = id.clone();
                        data.doc.update(move |x| {
                            x.update_display(value.clone());
                        });
                    })
                )).style(move |x| x.margin_left(retract as f32 * 13.0))
            },
        ).item_size_fixed(move || 20.0)
            .style(|s| s.flex_col().min_width_full().padding(6.0).flex_direction(FlexDirection::ColumnReverse)),
    )
        .style(|s| s.flex_grow(1.0).size_full())
        .scroll_style(|s| s.shrink_to_fit())
}
