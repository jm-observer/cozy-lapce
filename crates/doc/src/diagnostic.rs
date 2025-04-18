use std::sync::atomic::AtomicU64;

use floem::{
    prelude::{RwSignal, SignalUpdate, SignalWith},
    reactive::Scope,
};
use lapce_xi_rope::{RopeDelta, spans::Spans};
use lsp_types::{Diagnostic, DiagnosticSeverity};

#[derive(Clone, Debug, Copy)]
pub struct DiagnosticData {
    pub expanded:     RwSignal<bool>,
    pub diagnostics:  RwSignal<im::Vector<Diagnostic>>,
    diagnostics_span: RwSignal<Spans<Diagnostic>>,
    pub id:           RwSignal<AtomicU64>,
}

impl DiagnosticData {
    pub fn new(cx: Scope) -> Self {
        DiagnosticData {
            expanded:         cx.create_rw_signal(true),
            diagnostics:      cx.create_rw_signal(im::Vector::new()),
            diagnostics_span: cx.create_rw_signal(Spans::default()),
            id:               cx.create_rw_signal(AtomicU64::new(0)),
        }
    }

    pub fn set_spans(&self, spans: Spans<Diagnostic>) {
        self.diagnostics_span.set(spans);
    }

    pub fn spans(&self) -> RwSignal<Spans<Diagnostic>> {
        self.diagnostics_span
    }

    pub fn spans_apply_shape(&self, delta: &RopeDelta) {
        self.diagnostics_span.update(|diagnostics| {
            // if is_debug {
            //     for (interval, diag) in diagnostics.iter() {
            //         if diag
            //             .severity
            //             .as_ref()
            //             .map(|x| *x == DiagnosticSeverity::ERROR)
            //             .unwrap_or(false)
            //         {
            //             warn!("{interval:?} {:?}", diag.code_description);
            //         }
            //     }
            // }
            diagnostics.apply_shape(delta);
            // if is_debug {
            //     for (interval, diag) in diagnostics.iter() {
            //         if diag
            //             .severity
            //             .as_ref()
            //             .map(|x| *x == DiagnosticSeverity::ERROR)
            //             .unwrap_or(false)
            //         {
            //             warn!("{interval:?} {:?}", diag.code_description);
            //         }
            //     }
            // }
        })
    }

    pub fn find_most_serious_diag_by_offset<A>(
        &self,
        offset: usize,
        map: impl Fn(DiagnosticSeverity, &Diagnostic) -> A,
    ) -> Option<A> {
        let mut severty = DiagnosticSeverity::INFORMATION;

        self.diagnostics_span.with_untracked(|x| {
            let mut final_diag = None;
            x.iter().for_each(|(range, diag)| {
                if range.contains(offset) {
                    let diag_severty =
                        diag.severity.unwrap_or(DiagnosticSeverity::INFORMATION);
                    if diag_severty < severty {
                        severty = diag_severty;
                        final_diag = Some(diag);
                    }
                }
            });
            final_diag.map(|x| map(severty, x))
        })
    }

    pub fn find_most_serious_diag_by_range(
        &self,
        limit_start: usize,
        limit_end: usize,
    ) -> Option<Diagnostic> {
        let mut severty = DiagnosticSeverity::INFORMATION;
        self.diagnostics_span.with_untracked(|x| {
            let mut final_diag = None;
            x.iter().for_each(|(range, diag)| {
                if limit_start <= range.start && range.end <= limit_end {
                    let diag_severty =
                        diag.severity.unwrap_or(DiagnosticSeverity::INFORMATION);
                    if diag_severty < severty {
                        severty = diag_severty;
                        final_diag = Some(diag);
                    }
                }
            });
            final_diag.cloned()
        })
    }
}
