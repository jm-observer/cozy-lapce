use std::ops::AddAssign;

use floem::{
    peniko::Color,
    reactive::{ReadSignal, RwSignal, Scope, SignalUpdate, batch}
};

use crate::lines::{buffer::Buffer, style::EditorStyle};

#[derive(Clone)]
pub struct Signals {
    pub(crate) show_indent_guide: SignalManager<(bool, Color)>,
    pub(crate) buffer_rev:        SignalManager<u64>,
    pub(crate) buffer:            SignalManager<Buffer>,
    pub(crate) pristine:          SignalManager<bool>,
    // start from 1, (line num, paint width)
    pub(crate) last_line:         SignalManager<(usize, f64)>,
    pub paint_content:            SignalManager<usize>,
    pub max_width: SignalManager<f64>
}

impl Signals {
    pub fn new(
        cx: Scope,
        style: &EditorStyle,
        buffer: Buffer,
        last_line: (usize, f64)
    ) -> Self {
        let show_indent_guide = SignalManager::new(
            cx,
            (style.show_indent_guide(), style.indent_guide())
        );
        let rev = buffer.rev();
        let pristine = buffer.is_pristine();
        let buffer_rev = SignalManager::new(cx, rev);
        let buffer = SignalManager::new(cx, buffer);
        let last_line = SignalManager::new(cx, last_line);
        let pristine = SignalManager::new(cx, pristine);
        let paint_context = SignalManager::new(cx, 0usize);
        let max_width = SignalManager::new(cx, 0.0);

        Self {
            show_indent_guide,
            buffer_rev,
            buffer,
            last_line,
            pristine,
            paint_content: paint_context,
            max_width
        }
    }

    // pub fn update_buffer(&mut self, buffer: Buffer) {
    //     self.buffer_rev.update_if_not_equal(buffer.rev());
    //     self.buffer.update_force(buffer);
    // }

    pub fn signal_buffer_rev(&self) -> ReadSignal<u64> {
        self.buffer_rev.signal()
    }

    pub fn trigger(&mut self) {
        batch(|| {
            self.show_indent_guide.trigger();
            self.buffer_rev.trigger();
            self.buffer.trigger();
            self.last_line.trigger();
            self.pristine.trigger();
            self.paint_content.trigger();
            self.max_width.trigger();
        });
    }

    pub fn trigger_force(&mut self) {
        batch(|| {
            self.show_indent_guide.trigger_force();
            self.buffer_rev.trigger_force();
            self.buffer.trigger_force();
            self.last_line.trigger_force();
            self.paint_content.trigger_force();
            self.max_width.trigger_force();
        });
    }

    pub fn update_paint_text(&mut self) {
        self.paint_content.val_mut().add_assign(1);
    }
}

#[derive(Clone)]
pub struct SignalManager<V: Clone + 'static> {
    v:      V,
    signal: RwSignal<V>,
    dirty:  bool
}

impl<V: Clone + 'static> SignalManager<V> {
    pub fn new(cx: Scope, v: V) -> Self {
        Self {
            signal: cx.create_rw_signal(v.clone()),
            v,
            dirty: false
        }
    }

    pub fn update_force(&mut self, nv: V) {
        self.v = nv;
        self.dirty = true;
    }

    pub fn trigger(&mut self) {
        if self.dirty {
            self.signal.set(self.v.clone());
            self.dirty = false;
        }
    }

    pub fn trigger_force(&mut self) {
        self.signal.set(self.v.clone());
        self.dirty = false;
    }

    pub fn signal(&self) -> ReadSignal<V> {
        self.signal.read_only()
    }

    pub fn val(&self) -> &V {
        &self.v
    }

    pub fn val_mut(&mut self) -> &mut V {
        self.dirty = true;
        &mut self.v
    }
}

impl<V: Clone + PartialEq + 'static> SignalManager<V> {
    pub fn update_if_not_equal(&mut self, nv: V) -> bool {
        if self.v != nv {
            self.v = nv;
            self.dirty = true;
            true
        } else {
            false
        }
    }
}
