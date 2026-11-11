//! A surface you draw a loop on: press, circle something, let go. It sends the loop's points, in
//! window coordinates, and the app decides what was circled. A press with no real loop cancels.

use std::rc::Rc;

use vui_core::{Color, Corners, CursorIcon, Event, Fill, Point, PointerButton, Quad, Rect, Tone, rect};
use vui_runtime::{AccessInfo, Base, Dirty, EventCx, PaintCx, Role, Widget, WidgetEvent, builder};

/// Points closer than this to the last one are dropped.
const STEP: f32 = 3.0;
/// A loop smaller than this either way is a click, not a selection.
const SMALLEST: f32 = 12.0;
const STROKE: f32 = 3.0;

type Done<M> = Rc<dyn Fn(Vec<Point>) -> M>;

/// The widget behind [`Lasso`].
pub struct LassoWidget<M> {
    on_done: Option<Done<M>>,
    on_cancel: Option<Rc<dyn Fn() -> M>>,
    stroke: Fill,
    dim: Color,
}

#[derive(Default)]
pub struct LassoState {
    points: Vec<Point>,
    drawing: bool,
}

/// The smallest rect holding every point.
pub fn bounds(points: &[Point]) -> Rect {
    let Some(first) = points.first() else { return Rect::ZERO };
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    rect(x0, y0, x1 - x0, y1 - y0)
}

impl<M: 'static> Widget<M> for LassoWidget<M> {
    type State = LassoState;

    fn create_state(&self) -> LassoState {
        LassoState::default()
    }

    fn update(&self, old: &LassoWidget<M>, _state: &mut LassoState) -> Dirty {
        if self.stroke != old.stroke || self.dim != old.dim { Dirty::Paint } else { Dirty::None }
    }

    fn paint(&self, state: &LassoState, cx: &mut PaintCx) {
        if self.dim.a > 0.0 {
            cx.fill_rect(cx.rect, self.dim);
        }
        let color = cx.color(self.stroke);
        let half = STROKE / 2.0;
        // A chain of round dabs, filled in between so fast strokes stay unbroken.
        for pair in state.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let n = ((b.x - a.x).hypot(b.y - a.y) / (half * 0.8)).ceil().max(1.0) as usize;
            for i in 0..n {
                let t = i as f32 / n as f32;
                let p = Point { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
                cx.quad(Quad::new(rect(p.x - half, p.y - half, STROKE, STROKE)).radii(Corners::all(half)).fill(color));
            }
        }
    }

    fn event(&self, state: &mut LassoState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.points = vec![*pos];
                state.drawing = true;
                cx.capture_pointer();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerDown { .. }) => {
                if let Some(cancel) = &self.on_cancel {
                    cx.emit(cancel());
                }
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.drawing => {
                let last = *state.points.last().expect("drawing has a first point");
                if (pos.x - last.x).hypot(pos.y - last.y) >= STEP {
                    state.points.push(*pos);
                    cx.request_paint();
                }
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) if state.drawing => {
                state.drawing = false;
                cx.release_pointer();
                let points = std::mem::take(&mut state.points);
                let area = bounds(&points);
                let message = if area.w >= SMALLEST && area.h >= SMALLEST {
                    self.on_done.as_ref().map(|done| done(points))
                } else {
                    self.on_cancel.as_ref().map(|cancel| cancel())
                };
                if let Some(m) = message {
                    cx.emit(m);
                }
                cx.request_paint();
                true
            }
            _ => false,
        }
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Crosshair)
    }

    fn access(&self, _state: &LassoState, info: &mut AccessInfo) {
        info.role = Some(Role::Group);
    }
}

/// A surface to circle things on; size it over what can be circled.
pub struct Lasso<M: 'static> {
    base: Base<M>,
    widget: LassoWidget<M>,
}

builder!(Lasso<M> => LassoWidget<M>);

impl<M: 'static> Lasso<M> {
    /// What to send when a loop is drawn: its points, in window coordinates.
    pub fn on_done(mut self, f: impl Fn(Vec<Point>) -> M + 'static) -> Self {
        self.widget.on_done = Some(Rc::new(f));
        self
    }

    /// What to send for a click, a scribble too small to be a loop, or another button.
    pub fn on_cancel(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.widget.on_cancel = Some(Rc::new(f));
        self
    }

    pub fn stroke(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.stroke = fill.into();
        self
    }

    /// A wash over everything under the surface while it is up.
    pub fn dim(mut self, color: Color) -> Self {
        self.widget.dim = color;
        self
    }
}

pub fn lasso<M: 'static>() -> Lasso<M> {
    Lasso { base: Base::new(), widget: LassoWidget { on_done: None, on_cancel: None, stroke: Tone::Accent.into(), dim: Color::TRANSPARENT } }
}
