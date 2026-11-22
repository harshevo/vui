//! The histogram as a widget: lit bars for the chosen range, dim ones outside it, a line for the
//! marked value, and the bar under the pointer reported.

use std::rc::Rc;

use vui_core::{Corners, Event, Fill, Size, Tone, rect, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, EventCx, PaintCx, Role, Widget, WidgetEvent, builder};

use super::{HEIGHT, Histogram, bar, bin_at, plot};

type Hover<M> = Rc<dyn Fn(Option<usize>) -> M>;

pub struct HistogramWidget<M> {
    data: Rc<Histogram>,
    label: String,
    /// The tone of the lit bars: the accent, which suits a histogram over a slider.
    tone: Tone,
    on_hover: Option<Hover<M>>,
}

#[derive(Default)]
pub struct HistogramState {
    hover: Option<usize>,
}

impl<M: 'static> Widget<M> for HistogramWidget<M> {
    type State = HistogramState;

    fn create_state(&self) -> HistogramState {
        HistogramState::default()
    }

    fn update(&self, old: &Self, state: &mut HistogramState) -> Dirty {
        state.hover = state.hover.filter(|i| *i < self.data.counts.len());
        if self.tone == old.tone && (Rc::ptr_eq(&self.data, &old.data) || *self.data == *old.data) { Dirty::None } else { Dirty::Paint }
    }

    fn measure(&self, _state: &mut HistogramState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            Avail::MinContent => 120.0,
            Avail::MaxContent => 240.0,
        });
        size(width, known.y.unwrap_or(HEIGHT))
    }

    fn paint(&self, state: &HistogramState, cx: &mut PaintCx) {
        let area = cx.content;
        let h = &*self.data;
        cx.fill_rect(rect(area.x + super::INSET, area.bottom() - 1.0, plot(area).w, 1.0), Fill::Tone(Tone::Line, 1.0));
        for i in 0..h.counts.len() {
            let r = bar(h, area, i);
            if r.h <= 0.0 {
                continue;
            }
            let lit = h.is_lit(i);
            let hot = state.hover == Some(i);
            let fill = match (lit, hot) {
                (true, true) => Fill::Tone(self.tone, 1.0),
                (true, false) => Fill::Tone(self.tone, 0.78),
                (false, true) => Fill::Tone(Tone::Text3, 0.55),
                (false, false) => Fill::Tone(Tone::Text3, 0.28),
            };
            // Round the top only: the bars stand on a line.
            cx.fill_rounded(r, Corners { tl: 2.0, tr: 2.0, bl: 0.0, br: 0.0 }, fill);
        }
        if let Some(m) = h.marker {
            let p = plot(area);
            let x = p.x + p.w * h.fraction(m);
            cx.fill_rect(rect(x - 0.75, area.y, 1.5, area.h), Tone::Text);
            cx.fill_rounded(rect(x - 3.0, area.y - 1.0, 6.0, 6.0), Corners::all(3.0), Tone::Text);
        }
    }

    fn event(&self, state: &mut HistogramState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let now = match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => bin_at(&self.data, cx.content, pos.x).filter(|i| self.data.counts[*i] > 0),
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => None,
            _ => return false,
        };
        if now != state.hover {
            state.hover = now;
            if let Some(f) = &self.on_hover {
                cx.emit(f(now));
            }
            cx.request_paint();
        }
        false
    }

    fn access(&self, _state: &HistogramState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        let h = &*self.data;
        info.label = Some(if self.label.is_empty() { "Distribution".to_string() } else { self.label.clone() });
        info.value = Some(format!("{} of {} in the chosen range", h.lit_count(), h.total()));
    }
}

pub struct HistogramView<M: 'static> {
    base: Base<M>,
    widget: HistogramWidget<M>,
}

builder!(HistogramView<M> => HistogramWidget<M>);

impl<M: 'static> HistogramView<M> {
    /// What assistive technology calls it.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// The tone of the lit bars. The accent by default, which says "this is what you can move"; a
    /// picture that can't be acted on, such as a spread shown for reading, takes a data tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.widget.tone = tone;
        self
    }

    /// Sends the number of the bar under the pointer as it changes (`None` over nothing).
    pub fn on_hover(mut self, f: impl Fn(Option<usize>) -> M + 'static) -> Self {
        self.widget.on_hover = Some(Rc::new(f));
        self
    }
}

/// A histogram that fills the width it is given, 56 points tall unless told otherwise.
pub fn histogram<M: 'static>(data: Rc<Histogram>) -> HistogramView<M> {
    HistogramView { base: Base::new(), widget: HistogramWidget { data, label: String::new(), tone: Tone::Accent, on_hover: None } }
}
