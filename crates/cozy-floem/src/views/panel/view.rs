use floem::{
    Renderer, View, ViewId,
    context::{PaintCx, StyleCx},
    event::{Event, EventListener},
    keyboard::Key,
    kurbo::{Line, Point, Rect, Stroke},
    peniko::Color,
    prelude::Decorators,
    reactive::SignalGet,
    style::{CursorStyle, Style},
    taffy::NodeId,
    views::scroll
};
use log::error;

use crate::views::panel::data::DocManager;

pub fn panel(doc: DocManager) -> impl View {
    let hover_hyperlink = doc.with_untracked(|x| x.hover_hyperlink);
    let view = doc
        .on_event_cont(EventListener::PointerDown, move |event| {
            if let Event::PointerDown(pointer_event) = event {
                let rs = doc.try_update(|x| x.pointer_down(pointer_event.clone()));
                match rs {
                    Some(Err(err)) => error!("{err:?}"),
                    None => error!("doc try update point down fail"),
                    _ => ()
                }
            }
        })
        .on_event_cont(EventListener::PointerMove, move |event| {
            if let Event::PointerMove(pointer_event) = event {
                let rs = doc.try_update(|x| x.pointer_move(pointer_event.clone()));
                match rs {
                    Some(Err(err)) => error!("{err:?}"),
                    None => error!("doc try update point move fail"),
                    _ => ()
                }
            }
        })
        .on_event_cont(EventListener::PointerUp, move |event| {
            if let Event::PointerUp(pointer_event) = event {
                let rs = doc.try_update(|x| x.pointer_up(pointer_event.clone()));
                match rs {
                    Some(Err(err)) => error!("{err:?}"),
                    None => error!("doc try update point up fail"),
                    _ => ()
                }
            }
        })
        .keyboard_navigable()
        .on_key_down(
            Key::Character("c".into()),
            |modifiers| modifiers.control(),
            move |_| {
                doc.with_untracked(|x| {
                    if let Err(err) = x.copy_select() {
                        error!("{err:?}");
                    }
                });
            }
        )
        .style(move |x| {
            let hover_hyperlink = hover_hyperlink.get();
            x.apply_if(hover_hyperlink.is_some(), |x| {
                x.cursor(CursorStyle::Pointer)
            })
        });
    let handle_thickness = 12.0;
    scroll(view)
        .on_event_cont(EventListener::PointerDown, move |x| {
            if let Event::PointerDown(_) = x {
                doc.update(|x| {
                    x.auto_scroll = false;
                });
            }
        })
        .on_scroll(move |viewport| {
            doc.update(|x| x.update_viewport_by_scroll(viewport));
        })
        .scroll_style(move |s| s.handle_thickness(handle_thickness).shrink_to_fit())
        .style(move |x| {
            x.border(1.0)
                .padding_right(handle_thickness)
                // .margin_left(6.0)
                .padding_left(6.0)
                .padding_bottom(handle_thickness)
        })
}

#[allow(dead_code)]
pub struct Panel {
    pub id:         ViewId,
    pub inner_node: Option<NodeId>,
    pub doc:        DocManager
}

impl View for DocManager {
    fn id(&self) -> ViewId {
        self.panel_id
    }

    fn style_pass(&mut self, cx: &mut StyleCx<'_>) {
        cx.app_state_mut().request_paint(self.id());
    }

    fn update(
        &mut self,
        _cx: &mut floem::context::UpdateCx,
        _state: Box<dyn std::any::Any>
    ) {
    }

    fn layout(
        &mut self,
        cx: &mut floem::context::LayoutCx
    ) -> floem::taffy::prelude::NodeId {
        cx.layout_node(self.panel_id, true, |_cx| {
            if self.inner_node.is_none() {
                self.inner_node = Some(self.panel_id.new_taffy_node());
            }
            let view_size = self.with_untracked(|x| x.view_size());
            // debug!("layout view_size={view_size:?}");
            let inner_node = self.inner_node.unwrap();
            let style = Style::new()
                .width(view_size.width)
                .height(view_size.height)
                .to_taffy_style();
            self.panel_id.set_taffy_style(inner_node, style);

            vec![inner_node]
        })
    }

    fn compute_layout(
        &mut self,
        _cx: &mut floem::context::ComputeLayoutCx
    ) -> Option<Rect> {
        // let viewport = cx.current_viewport();
        // self.editor.doc().lines.update(|x| {
        //     if let Err(err) = x.update_viewport_size(viewport) {
        //         error!("{err:?}");
        //     }
        // });
        None
    }

    fn paint(&mut self, cx: &mut PaintCx) {
        // debug!("paint");
        let (viewport, lines, position_of_cursor, selections, style) = self
            .with_untracked(|x| {
                (
                    x.viewport,
                    x.viewport_lines(),
                    x.position_of_cursor(),
                    x.select_of_cursor(),
                    x.style.clone()
                )
            });
        match selections {
            Ok(rects) => {
                for rect in rects {
                    let rect = viewport.intersect(rect);
                    if !rect.is_zero_area() {
                        cx.fill(&rect, style.selection_bg, 0.0);
                    }
                }
            },
            Err(err) => {
                error!("{err:?}");
            }
        }
        // paint cursor
        match position_of_cursor {
            Ok(Some(rect)) => {
                let rect = viewport.intersect(rect);
                if !rect.is_zero_area() {
                    cx.fill(&rect, Color::BLACK, 0.0);
                }
            },
            Err(err) => {
                error!("{err:?}");
            },
            Ok(None) => {}
        }
        for line_info in lines {
            let y = line_info.pos_y;
            // debug!("line_index={} y={y} ", line_info.line_index);
            paint_extra_style(cx, &line_info.hyperlinks);
            cx.draw_text_with_layout(
                line_info.text.layout_runs(),
                Point::new(0.0, y)
            );
        }
        // paint select
    }
}

fn paint_extra_style(cx: &mut PaintCx, extra_styles: &[(Point, Point, Color)]) {
    for (start, end, color) in extra_styles {
        cx.stroke(&Line::new(*start, *end), color, &Stroke::new(0.5));
    }
}
