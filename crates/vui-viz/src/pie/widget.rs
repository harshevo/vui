//! The pie as a widget: slices draw in round the circle when it first appears, the one under the
//! pointer (or its legend row) moves out while the others fade back, and a donut writes the hovered
//! slice's value in its hole.

use std::rc::Rc;

use vui_core::{Event, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, EventCx, PaintCx, Role, TickCx, Widget, WidgetEvent, builder};

use super::{Pie, draw, layout, slice_at};
use crate::canvas::{Canvas, paint};
use crate::format::Format;
use crate::style::VizStyle;

const REVEAL_SECONDS: f32 = 0.8;
/// How quickly a slice moves out and back; bigger is quicker.
const EASE: f32 = 16.0;

type Select<M> = Rc<dyn Fn(Option<usize>) -> M>;

pub struct PieWidget<M> {
    pie: Rc<Pie>,
    label: String,
    animate: bool,
    on_hover: Option<Select<M>>,
}

pub struct PieState {
    reveal: f32,
    hover: Option<usize>,
    lift: Vec<f32>,
}

impl PieState {
    fn settled(&self) -> bool {
        self.reveal >= 1.0 && self.lift.iter().enumerate().all(|(i, l)| *l == if self.hover == Some(i) { 1.0 } else { 0.0 })
    }
}

impl<M: 'static> Widget<M> for PieWidget<M> {
    type State = PieState;

    fn create_state(&self) -> PieState {
        PieState { reveal: if self.animate { 0.0 } else { 1.0 }, hover: None, lift: vec![0.0; self.pie.slices.len()] }
    }

    fn update(&self, old: &PieWidget<M>, state: &mut PieState) -> Dirty {
        state.lift.resize(self.pie.slices.len(), 0.0);
        state.hover = state.hover.filter(|i| *i < self.pie.slices.len());
        if Rc::ptr_eq(&self.pie, &old.pie) || *self.pie == *old.pie { Dirty::None } else { Dirty::Layout }
    }

    fn measure(&self, _state: &mut PieState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            // It can be drawn narrow, so it never holds a row open at its usual width.
            Avail::MinContent => 160.0,
            Avail::MaxContent => 360.0,
        });
        let rows = self.pie.slices.len() as f32 * 22.0;
        // Beside a circle of up to 220 points when there is room, otherwise the legend goes under it.
        let height = if !self.pie.legend {
            width.min(240.0)
        } else if width >= 360.0 {
            rows.clamp(200.0, 240.0)
        } else {
            width.min(220.0) + 12.0 + rows
        };
        size(width, known.y.unwrap_or(height))
    }

    fn paint(&self, state: &PieState, cx: &mut PaintCx) {
        let style = VizStyle::from_palette(cx.palette());
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        draw(&mut canvas, &self.pie, cx.content, &style, state.reveal, &state.lift, state.hover);
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
    }

    fn event(&self, state: &mut PieState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let over = match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                let style = VizStyle::from_palette(cx.env.palette());
                slice_at(&self.pie, &layout(cx.env.fonts(), &self.pie, cx.content, &style), *pos)
            }
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => None,
            _ => return false,
        };
        if over != state.hover {
            state.hover = over;
            if let Some(f) = &self.on_hover {
                cx.emit(f(over));
            }
            cx.request_frame();
            cx.request_paint();
        }
        false
    }

    fn tick(&self, state: &mut PieState, _cx: &mut TickCx, dt: f32) -> bool {
        state.reveal = (state.reveal + dt / REVEAL_SECONDS).min(1.0);
        let k = 1.0 - (-dt * EASE).exp();
        let hover = state.hover;
        for (i, l) in state.lift.iter_mut().enumerate() {
            let to = if hover == Some(i) { 1.0 } else { 0.0 };
            *l += (to - *l) * k;
            if (to - *l).abs() < 0.01 {
                *l = to;
            }
        }
        !state.settled()
    }

    fn animating(&self, state: &PieState) -> bool {
        !state.settled()
    }

    fn access(&self, _state: &PieState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        let parts: Vec<String> =
            (0..self.pie.slices.len()).map(|i| format!("{} {}", self.pie.slices[i].label, Format::Percent(0).apply(self.pie.share(i)))).collect();
        info.label = Some(if self.label.is_empty() { format!("Pie chart: {}", parts.join(", ")) } else { self.label.clone() });
    }
}

pub struct PieView<M: 'static> {
    base: Base<M>,
    widget: PieWidget<M>,
}

builder!(PieView<M> => PieWidget<M>);

impl<M: 'static> PieView<M> {
    /// What assistive technology reads out; by default every slice and its share.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// Skip the draw-in when the pie first appears.
    pub fn still(mut self) -> Self {
        self.widget.animate = false;
        self
    }

    /// Sends the index of the slice under the pointer as it changes, and `None` when it leaves.
    pub fn on_hover(mut self, f: impl Fn(Option<usize>) -> M + 'static) -> Self {
        self.widget.on_hover = Some(Rc::new(f));
        self
    }
}

/// A pie or donut that fills the width it is given: `pie(Rc::new(Pie::new().slice("Direct", 42.0).donut()))`.
pub fn pie<M: 'static>(data: Rc<Pie>) -> PieView<M> {
    PieView { base: Base::new(), widget: PieWidget { pie: data, label: String::new(), animate: true, on_hover: None } }
}
