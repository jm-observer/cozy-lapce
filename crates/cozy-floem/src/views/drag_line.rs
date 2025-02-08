use floem::{
    View,
    event::{Event, EventListener},
    kurbo::Point,
    prelude::{
        Decorators, RwSignal, SignalGet, SignalUpdate, create_rw_signal, empty
    },
    style::CursorStyle
};

pub fn x_drag_line(left_width: RwSignal<f64>) -> impl View {
    let view = empty();
    let view_id = view.id();
    let drag_start: RwSignal<Option<Point>> = create_rw_signal(None);
    view.on_event_stop(EventListener::PointerDown, move |event| {
        view_id.request_active();
        if let Event::PointerDown(pointer_event) = event {
            drag_start.set(Some(pointer_event.pos));
        }
    })
    .on_event_stop(EventListener::PointerMove, move |event| {
        if let Event::PointerMove(pointer_event) = event {
            if drag_start.get_untracked().is_some() && pointer_event.pos.x != 0.0 {
                left_width.set(pointer_event.pos.x + left_width.get_untracked());
            }
        }
    })
    .on_event_stop(EventListener::PointerUp, move |_| {
        drag_start.set(None);
    })
    .style(|x| x.hover(|x| x.cursor(CursorStyle::ColResize)))
    .debug_name("drag_line")
}
