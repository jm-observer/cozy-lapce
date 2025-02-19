use std::{cmp::Ordering, rc::Rc};

use doc::lines::mode::Modes;
use floem::{
    View,
    event::{Event, EventListener},
    prelude::text_input,
    reactive::{
        Memo, RwSignal, SignalGet, SignalUpdate, SignalWith,
        create_effect, create_memo, create_rw_signal, create_signal
    },
    style::CursorStyle,
    views::{
        Decorators, VirtualDirection, VirtualItemSize, container, dyn_stack, label,
        scroll, stack, text, virtual_stack
    }
};
use itertools::Itertools;

use crate::{
    command::LapceCommand,
    config::{color::LapceColor},
    keypress::{
        KeyPressData,
        keymap::{KeyMap, KeyMapPress}
    },
    window_workspace::CommonData
};
use crate::config::WithLapceConfig;

#[derive(Clone)]
pub struct KeymapPicker {
    cmd:    RwSignal<Option<LapceCommand>>,
    keymap: RwSignal<Option<KeyMap>>,
    keys:   RwSignal<Vec<(KeyMapPress, bool)>>
}

pub fn keymap_view(common: Rc<CommonData>) -> impl View {
    let config = common.config;
    let keypress = common.keypress;
    let ui_line_height_memo = common.ui_line_height;
    let ui_line_height = move || ui_line_height_memo.get() * 1.2;
    let modal = create_memo(move |_| config.with(|x| x.core.modal));
    let picker = KeymapPicker {
        cmd:    create_rw_signal(None),
        keymap: create_rw_signal(None),
        keys:   create_rw_signal(Vec::new())
    };

    // let cx = Scope::current();
    // let text_input_view = TextInputBuilder::new().build(cx, editors,
    // common.clone()); let doc = text_input_view.doc_signal();
    let (read_order, write_order) = create_signal(KeyMapOrder::default());

    let query_str = create_rw_signal(String::new());
    let key_map_items = move || {
        let pattern = query_str.get();
        let keypress = keypress.get();
        let items = keypress.commands_with_keymap.iter().filter_map(|keymap| {
            let cmd = keypress.commands.get(&keymap.command).cloned()?;

            let cmd_name_contains_pattern =
                cmd.kind.str().replace('_', " ").contains(&pattern);
            let cmd_desc_contains_pattern = cmd
                .kind
                .desc()
                .map(|desc| desc.to_lowercase().contains(&pattern))
                .unwrap_or(false);
            let shortcut_contains_pattern = keymap
                .key
                .iter()
                .any(|k| k.label().trim().to_lowercase().contains(&pattern));
            let when_contains_pattern = keymap
                .when
                .as_ref()
                .map(|when| when.to_lowercase().contains(&pattern))
                .unwrap_or(false);

            if cmd_name_contains_pattern
                || cmd_desc_contains_pattern
                || shortcut_contains_pattern
                || when_contains_pattern
            {
                Some((cmd, Some(keymap.clone())))
            } else {
                None
            }
        });
        let mut items = match read_order.get() {
            KeyMapOrder::None => {
                items.collect::<im::Vector<(LapceCommand, Option<KeyMap>)>>()
            },
            KeyMapOrder::OrderKey => items
                .sorted_by(|x, y| {
                    match (
                        x.1.as_ref().and_then(|x| x.key.first()),
                        y.1.as_ref().and_then(|x| x.key.first())
                    ) {
                        (Some(x_key), Some(y_key)) => x_key.cmp(y_key),
                        (Some(_), None) => Ordering::Greater,
                        (None, Some(_)) => Ordering::Less,
                        _ => Ordering::Equal
                    }
                })
                .collect::<im::Vector<(LapceCommand, Option<KeyMap>)>>()
        };
        items.extend(keypress.commands_without_keymap.iter().filter_map(|cmd| {
            let match_pattern = cmd.kind.str().replace('_', " ").contains(&pattern)
                || cmd
                    .kind
                    .desc()
                    .map(|desc| desc.to_lowercase().contains(&pattern))
                    .unwrap_or(false);
            if !match_pattern {
                return None;
            }
            Some((cmd.clone(), None))
        }));
        items
            .into_iter()
            .enumerate()
            .collect::<im::Vector<(usize, (LapceCommand, Option<KeyMap>))>>()
    };

    let view_fn =
        move |(i, (cmd, keymap)): (usize, (LapceCommand, Option<KeyMap>))| {
            let local_keymap = keymap.clone();
            let local_cmd = cmd.clone();
            stack((
                container(
                    text(
                        cmd.kind
                            .desc()
                            .map(|desc| desc.to_string())
                            .unwrap_or_else(|| cmd.kind.str().replace('_', " "))
                    )
                    .style(|s| {
                        s.text_ellipsis()
                            .absolute()
                            .items_center()
                            .min_width(0.0)
                            .padding_horiz(10.0)
                            .size_pct(100.0, 100.0)
                    })
                )
                .style(move |s| {
                    s.height_pct(100.0)
                        .min_width(0.0)
                        .flex_basis(0.0)
                        .flex_grow(1.0)
                        .border_right(1.0)
                        .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                })
                .debug_name("command kind"),
                {
                    let keymap = keymap.clone();
                    dyn_stack(
                        move || {
                            keymap
                                .as_ref()
                                .map(|keymap| {
                                    keymap
                                        .key
                                        .iter()
                                        .map(|key| key.label())
                                        .filter(|l| !l.is_empty())
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default()
                        },
                        |k| k.clone(),
                        move |key| {
                            text(key.clone()).style(move |s| {
                                s.padding_horiz(5.0)
                                    .padding_vert(1.0)
                                    .margin_right(5.0)
                                    .border(1.0)
                                    .border_radius(3.0)
                                    .border_color(
                                        config.with_color(LapceColor::LAPCE_BORDER)
                                    )
                            })
                        }
                    )
                    .style(move |s| {
                        s.items_center()
                            .padding_horiz(10.0)
                            .min_width(200.0)
                            .height_pct(100.0)
                            .border_right(1.0)
                            .border_color(
                                config.with_color(LapceColor::LAPCE_BORDER)
                            )
                    })
                    .debug_name("keymaps")
                },
                {
                    let keymap = keymap.clone();
                    let bits = [
                        (Modes::INSERT, "Insert"),
                        (Modes::NORMAL, "Normal"),
                        (Modes::VISUAL, "Visual"),
                        (Modes::TERMINAL, "Terminal")
                    ];
                    let modes = keymap
                        .as_ref()
                        .map(|keymap| {
                            bits.iter()
                                .filter_map(|(bit, mode)| {
                                    if keymap.modes.contains(*bit) {
                                        Some(mode.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    dyn_stack(
                        move || modes.clone(),
                        |m| m.clone(),
                        move |mode| {
                            text(mode.clone()).style(move |s| {
                                s.padding_horiz(5.0)
                                    .padding_vert(1.0)
                                    .margin_right(5.0)
                                    .border(1.0)
                                    .border_radius(3.0)
                                    .border_color(
                                        config.with_color(LapceColor::LAPCE_BORDER)
                                    )
                            })
                        }
                    )
                    .style(move |s| {
                        s.items_center()
                            .padding_horiz(10.0)
                            .min_width(200.0)
                            .height_pct(100.0)
                            .border_right(1.0)
                            .border_color(
                                config.with_color(LapceColor::LAPCE_BORDER)
                            )
                            .apply_if(!modal.get(), |s| s.hide())
                    })
                },
                container(
                    text(
                        keymap
                            .as_ref()
                            .and_then(|keymap| keymap.when.clone())
                            .unwrap_or_default()
                    )
                    .style(|s| {
                        s.text_ellipsis()
                            .absolute()
                            .items_center()
                            .min_width(0.0)
                            .padding_horiz(10.0)
                            .size_pct(100.0, 100.0)
                    })
                )
                .style(move |s| {
                    s.height_pct(100.0)
                        .min_width(0.0)
                        .flex_basis(0.0)
                        .flex_grow(1.0)
                })
            ))
            .on_click_stop(move |_| {
                let keymap = if let Some(keymap) = local_keymap.clone() {
                    keymap
                } else {
                    KeyMap {
                        command: local_cmd.kind.str().to_string(),
                        key:     Vec::new(),
                        modes:   Modes::empty(),
                        when:    None
                    }
                };
                picker.keymap.set(Some(keymap));
                picker.cmd.set(Some(local_cmd.clone()));
                picker.keys.update(|keys| {
                    keys.clear();
                });
            })
            .style(move |s| {
                let (bg, border_color) = config.with(|config| {
                    (
                        config.color(LapceColor::EDITOR_CURRENT_LINE)
                        , config.color(LapceColor::LAPCE_BORDER)
                    )
                });
                s.items_center()
                    .height(ui_line_height() as f32)
                    .width_pct(100.0)
                    .apply_if(i % 2 > 0, |s| {
                        s.background(bg)
                    })
                    .border_bottom(1.0)
                    .border_color(border_color)
            })
        };

    stack((
        container(
            text_input(query_str)
                .placeholder("Search Key Bindings")
                .keyboard_navigable()
                .style(move |s| {
                    s.width_pct(100.0)
                        .border_radius(2.0)
                        .border(1.0)
                        .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                    // }).pointer_down(move || {
                    //     focus.set(Focus::Panel(PanelKind::Plugin));
                })
        )
        .style(|s| s.padding_bottom(10.0).width_pct(100.0)),
        stack((
            container(text("Command").style(move |s| {
                s.text_ellipsis().padding_horiz(10.0).min_width(0.0)
            }))
            .style(move |s| {
                s.items_center()
                    .height_pct(100.0)
                    .min_width(0.0)
                    .flex_basis(0.0)
                    .flex_grow(1.0)
                    .border_right(1.0)
                    .border_color(config.with_color(LapceColor::LAPCE_BORDER))
            }),
            container(text("Key Binding"))
                .style(move |s| {
                    s.width(200.0)
                        .items_center()
                        .padding_horiz(10.0)
                        .height_pct(100.0)
                        .border_right(1.0)
                        .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                })
                .on_click_stop(move |_| {
                    write_order.update(|x| x.update());
                }),
            text("Modes").style(move |s| {
                s.width(200.0)
                    .items_center()
                    .padding_horiz(10.0)
                    .height_pct(100.0)
                    .border_right(1.0)
                    .border_color(config.with_color(LapceColor::LAPCE_BORDER))
                    .apply_if(!modal.get(), |s| s.hide())
            }),
            container(text("When").style(move |s| {
                s.text_ellipsis().padding_horiz(10.0).min_width(0.0)
            }))
            .style(move |s| {
                s.items_center()
                    .height_pct(100.0)
                    .min_width(0.0)
                    .flex_basis(0.0)
                    .flex_grow(1.0)
            })
        ))
        .style(move |s| {
            let (bg, border_color) = config.with(|config| {
                (
                    config.color(LapceColor::EDITOR_CURRENT_LINE)
                    , config.color(LapceColor::LAPCE_BORDER)
                )
            });
            s.font_bold()
                .height(ui_line_height() as f32)
                .width_pct(100.0)
                .border_top(1.0)
                .border_bottom(1.0)
                .border_color(border_color)
                .background(bg)
        }),
        container(
            scroll(
                virtual_stack(
                    VirtualDirection::Vertical,
                    VirtualItemSize::Fixed(Box::new(ui_line_height)),
                    key_map_items,
                    |(i, (cmd, keymap)): &(
                        usize,
                        (LapceCommand, Option<KeyMap>)
                    )| { (*i, cmd.kind.str(), keymap.clone()) },
                    view_fn
                )
                .style(|s| s.flex_col().width_pct(100.0))
            )
            .style(|s| s.absolute().size_pct(100.0, 100.0))
        )
        .style(|s| s.width_pct(100.0).flex_basis(0.0).flex_grow(1.0)),
        keyboard_picker_view(picker, common.ui_line_height, config, common)
    ))
    .style(|s| {
        s.absolute()
            .size_pct(100.0, 100.0)
            .flex_col()
            .padding_top(20.0)
            .padding_left(20.0)
            .padding_right(20.0)
    })
    .debug_name("keymap view")
}

fn keyboard_picker_view(
    picker: KeymapPicker,
    ui_line_height: Memo<f64>,
    config: WithLapceConfig,
    common: Rc<CommonData>
) -> impl View {
    let picker_cmd = picker.cmd;
    let view = container(
        stack((
            label(move || {
                picker_cmd.with(|cmd| {
                    cmd.as_ref()
                        .map(|cmd| {
                            cmd.kind
                                .desc()
                                .map(|desc| desc.to_string())
                                .unwrap_or_else(|| cmd.kind.str().replace('_', " "))
                        })
                        .unwrap_or_default()
                })
            }),
            dyn_stack(
                move || {
                    picker
                        .keys
                        .get()
                        .iter()
                        .map(|(key, _)| key.label())
                        .filter(|l| !l.is_empty())
                        .enumerate()
                        .collect::<Vec<(usize, String)>>()
                },
                |(i, k)| (*i, k.clone()),
                move |(_, key)| {
                    text(key.clone()).style(move |s| {
                        s.padding_horiz(5.0)
                            .padding_vert(1.0)
                            .margin_right(5.0)
                            .height(ui_line_height.get() as f32)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(
                                config.with_color(LapceColor::LAPCE_BORDER)
                            )
                    })
                }
            )
            .style(move |s| {
                let (bg, border_color) = config.with(|config| {
                    (
                        config.color(LapceColor::EDITOR_CURRENT_LINE)
                        , config.color(LapceColor::LAPCE_BORDER)
                    )
                });
                s.items_center()
                    .justify_center()
                    .width_pct(100.0)
                    .margin_top(20.0)
                    .height((ui_line_height.get() as f32) * 1.2)
                    .border(1.0)
                    .border_radius(6.0)
                    .border_color(border_color)
                    .background(bg)
            }),
            stack((
                text("Save")
                    .style(move |s| {
                        let (lb, hbg, abg) = config.with(|config| {
                            (config.color(LapceColor::LAPCE_BORDER), config.color(LapceColor::PANEL_HOVERED_BACKGROUND), config.color(LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND))
                        });
                        s.width(100.0)
                            .justify_center()
                            .padding_vert(8.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(lb)
                            .hover(|s| {
                                s.cursor(CursorStyle::Pointer).background(
                                    hbg
                                )
                            })
                            .active(|s| {
                                s.background(abg)
                            })
                    })
                    .on_click_stop(move |_| {
                        let keymap = picker.keymap.get_untracked();
                        if let Some(keymap) = keymap {
                            let keys = picker.keys.get_untracked();
                            picker.keymap.set(None);
                            KeyPressData::update_file(
                                &keymap,
                                &keys
                                    .iter()
                                    .map(|(key, _)| key.clone())
                                    .collect::<Vec<KeyMapPress>>(),
                                common.clone()
                            );
                        }
                    }),
                text("Cancel")
                    .style(move |s| {
                        let (lb, hbg, abg) = config.with(|config| {
                            (config.color(LapceColor::LAPCE_BORDER), config.color(LapceColor::PANEL_HOVERED_BACKGROUND), config.color(LapceColor::PANEL_HOVERED_ACTIVE_BACKGROUND))
                        });
                        s.margin_left(20.0)
                            .width(100.0)
                            .justify_center()
                            .padding_vert(8.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(lb)
                            .hover(|s| {
                                s.cursor(CursorStyle::Pointer).background(hbg)
                            })
                            .active(|s| {
                                s.background(abg)
                            })
                    })
                    .on_click_stop(move |_| {
                        picker.keymap.set(None);
                    })
            ))
            .style(move |s| {
                s.items_center()
                    .justify_center()
                    .width_pct(100.0)
                    .margin_top(20.0)
                    .border_color(config.with_color(LapceColor::LAPCE_BORDER))
            })
        ))
        .on_event_stop(EventListener::PointerDown, |_| {})
        .style(move |s| {
            let (lb, hbg,) = config.with(|config| {
                (config.color(LapceColor::LAPCE_BORDER), config.color(LapceColor::PANEL_BACKGROUND))
            });
            s.items_center()
                .flex_col()
                .padding(20.0)
                .width(400.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(lb)
                .background(hbg)
        })
    )
    .keyboard_navigable()
    .on_event_stop(EventListener::KeyDown, move |event| {
        if let Event::KeyDown(key_event) = event {
            if let Some(keypress) = KeyPressData::keypress(key_event) {
                if let Some(keypress) = keypress.keymap_press() {
                    picker.keys.update(|keys| {
                        if let Some((last_key, last_key_confirmed)) = keys.last() {
                            if !*last_key_confirmed && last_key.is_modifiers() {
                                keys.pop();
                            }
                        }
                        if keys.len() == 2 {
                            keys.clear();
                        }
                        keys.push((keypress, false));
                    })
                }
            }
        }
    })
    .on_event_stop(EventListener::KeyUp, move |event| {
        if let Event::KeyUp(_key_event) = event {
            picker.keys.update(|keys| {
                if let Some((_last_key, last_key_confirmed)) = keys.last_mut() {
                    *last_key_confirmed = true;
                }
            })
        }
    })
    .style(move |s| {
        s.absolute()
            .size_pct(100.0, 100.0)
            .items_center()
            .justify_center()
            .apply_if(picker.keymap.with(|keymap| keymap.is_none()), |s| s.hide())
    })
    .debug_name("keyboard picker");

    let id = view.id();
    create_effect(move |_| {
        if picker.keymap.with(|k| k.is_some()) {
            id.request_focus();
        }
    });

    view
}

#[derive(Debug, Default, Clone)]
pub enum KeyMapOrder {
    #[default]
    None,
    OrderKey
}

impl KeyMapOrder {
    pub fn update(&mut self) {
        match self {
            KeyMapOrder::None => *self = Self::OrderKey,
            KeyMapOrder::OrderKey => *self = Self::None
        }
    }
}
