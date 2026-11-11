//! The chart as a widget: hover shows the values, dragging pans, ⌘-scroll or a pinch zooms, a double
//! click shows everything again, and the first time it appears the marks draw in from the left.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Key, KeyEvent, NamedKey, Point, PointerButton, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, TickCx, Widget,
    WidgetEvent, builder,
};

use super::{Chart, View, draw, draw_probe, layout, probe};
use crate::canvas::{Canvas, paint};
use crate::style::VizStyle;

const DEFAULT_WIDTH: f32 = 480.0;
/// How quickly the vertical range settles after the window moves; bigger is quicker.
const SETTLE: f64 = 14.0;
const REVEAL_SECONDS: f32 = 0.9;

type Hover<M> = Rc<dyn Fn(Option<f64>) -> M>;

pub struct ChartWidget<M> {
    chart: Rc<Chart>,
    label: String,
    animate: bool,
    /// A guide to show when the pointer is elsewhere: another chart's hover, for charts that move together.
    guide: Option<f64>,
    on_hover: Option<Hover<M>>,
}

pub struct ChartState {
    extent: (f64, f64),
    min_span: f64,
    /// What is on screen. `view.x` follows the pointer exactly; the vertical ranges glide toward their
    /// targets.
    view: View,
    target: View,
    reveal: f32,
    pointer: Option<Point>,
    drag: Option<Drag>,
}

struct Drag {
    from: f32,
    window: (f64, f64),
}

impl ChartState {
    fn new(chart: &Chart, animate: bool) -> ChartState {
        let extent = chart.x_extent();
        let view = chart.fit(extent);
        ChartState {
            extent,
            min_span: chart.min_span(),
            view,
            target: view,
            reveal: if animate { 0.0 } else { 1.0 },
            pointer: None,
            drag: None,
        }
    }

    /// Moves the window, keeping it inside the data, and aims the vertical ranges at what it now holds.
    fn set_window(&mut self, chart: &Chart, x: (f64, f64)) {
        let full = self.extent.1 - self.extent.0;
        let span = (x.1 - x.0).clamp(self.min_span.min(full), full);
        let lo =
            x.0.clamp(self.extent.0, (self.extent.1 - span).max(self.extent.0));
        self.view.x = (lo, lo + span);
        let fit = chart.fit(self.view.x);
        self.target.x = self.view.x;
        self.target.y = fit.y;
        self.target.strip = fit.strip;
    }

    fn zoom(&mut self, chart: &Chart, about: f64, factor: f64) {
        let (lo, hi) = self.view.x;
        let k = 1.0 / factor.max(0.05);
        self.set_window(chart, (about - (about - lo) * k, about + (hi - about) * k));
    }

    fn settled(&self) -> bool {
        self.view.y == self.target.y && self.view.strip == self.target.strip && self.reveal >= 1.0
    }
}

fn glide(from: (f64, f64), to: (f64, f64), k: f64) -> (f64, f64) {
    let step = |a: f64, b: f64| {
        if (b - a).abs() <= (b.abs() + a.abs()).max(1e-9) * 1e-5 {
            b
        } else {
            a + (b - a) * k
        }
    };
    (step(from.0, to.0), step(from.1, to.1))
}

impl<M> ChartWidget<M> {
    fn style(env: &Env) -> VizStyle {
        VizStyle::from_palette(env.palette())
    }
}

impl<M: 'static> Widget<M> for ChartWidget<M> {
    type State = ChartState;

    fn create_state(&self) -> ChartState {
        ChartState::new(&self.chart, self.animate)
    }

    fn update(&self, old: &ChartWidget<M>, state: &mut ChartState) -> Dirty {
        if !Rc::ptr_eq(&self.chart, &old.chart) && *self.chart != *old.chart {
            // New data: keep the window if it still fits, otherwise show everything.
            let window = state.view.x;
            state.extent = self.chart.x_extent();
            state.min_span = self.chart.min_span();
            let fits = window.0 >= state.extent.0
                && window.1 <= state.extent.1
                && window != old.chart.x_extent();
            state.set_window(&self.chart, if fits { window } else { state.extent });
            state.view.strip = state.target.strip;
        }
        Dirty::Paint
    }

    fn measure(
        &self,
        _state: &mut ChartState,
        _env: &Env,
        known: Pair<Option<f32>>,
        avail: Pair<Avail>,
    ) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            // It can be drawn narrow, so it never holds a row open at its usual width.
            Avail::MinContent => 160.0,
            Avail::MaxContent => DEFAULT_WIDTH,
        });
        size(width, known.y.unwrap_or((width * 0.52).clamp(160.0, 380.0)))
    }

    fn paint(&self, state: &ChartState, cx: &mut PaintCx) {
        let style = Self::style(cx.env);
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        let drawn = draw(
            &mut canvas,
            &self.chart,
            cx.content,
            &state.view,
            &style,
            state.reveal,
        );
        let at = state
            .pointer
            .filter(|p| drawn.plot.contains(*p))
            .map(|p| drawn.x.invert(p.x))
            .or(self.guide);
        if let Some(p) = at.and_then(|x| probe(&self.chart, &style, x)) {
            draw_probe(&mut canvas, &drawn, &p, &style);
        }
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
    }

    fn event(&self, state: &mut ChartState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let style = Self::style(cx.env);
        let plot = layout(cx.env.fonts(), &self.chart, cx.content, &state.view, &style).plot;
        let per_px = |s: &ChartState| (s.view.x.1 - s.view.x.0) / f64::from(plot.w.max(1.0));
        match event {
            WidgetEvent::Input(Event::PointerDown {
                pos,
                button: PointerButton::Primary,
                click_count,
                ..
            }) if plot.contains(*pos) => {
                if *click_count >= 2 {
                    state.set_window(&self.chart, state.extent);
                    cx.request_frame();
                } else {
                    state.drag = Some(Drag {
                        from: pos.x,
                        window: state.view.x,
                    });
                    cx.capture_pointer();
                }
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                if let Some(drag) = &state.drag {
                    let moved = -f64::from(pos.x - drag.from) * per_px(state);
                    let window = (drag.window.0 + moved, drag.window.1 + moved);
                    state.set_window(&self.chart, window);
                    cx.request_frame();
                }
                state.pointer = Some(*pos);
                self.tell(cx, state, plot.contains(*pos).then_some(pos.x));
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => state.drag.take().is_some(),
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => {
                state.pointer = None;
                self.tell(cx, state, None);
                cx.request_paint();
                false
            }
            WidgetEvent::Input(Event::Scroll { pos, delta, .. }) if plot.contains(*pos) => {
                if cx.mods.cmd || cx.mods.ctrl {
                    let about = state_x(state, plot.x, plot.w, pos.x);
                    state.zoom(&self.chart, about, (f64::from(delta.y) * 0.01).exp());
                } else if delta.x != 0.0 {
                    let moved = -f64::from(delta.x) * per_px(state);
                    state.set_window(
                        &self.chart,
                        (state.view.x.0 + moved, state.view.x.1 + moved),
                    );
                } else {
                    return false;
                }
                cx.request_frame();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Magnify { pos, delta, .. }) if plot.contains(*pos) => {
                let about = state_x(state, plot.x, plot.w, pos.x);
                state.zoom(&self.chart, about, 1.0 + f64::from(*delta));
                cx.request_frame();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Key(key)) => self.key(state, cx, key),
            _ => false,
        }
    }

    fn tick(&self, state: &mut ChartState, _cx: &mut TickCx, dt: f32) -> bool {
        state.reveal = (state.reveal + dt / REVEAL_SECONDS).min(1.0);
        let k = 1.0 - (-f64::from(dt) * SETTLE).exp();
        state.view.y = glide(state.view.y, state.target.y, k);
        state.view.strip = glide(state.view.strip, state.target.strip, k);
        !state.settled()
    }

    fn animating(&self, state: &ChartState) -> bool {
        !state.settled()
    }

    fn access(&self, _state: &ChartState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        let names: Vec<&str> = self.chart.layers.iter().map(|l| l.name.as_str()).collect();
        info.label = Some(if self.label.is_empty() {
            format!("Chart: {}", names.join(", "))
        } else {
            self.label.clone()
        });
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Crosshair)
    }
}

/// The data x under pixel `px`.
fn state_x(state: &ChartState, left: f32, width: f32, px: f32) -> f64 {
    let t = f64::from((px - left) / width.max(1.0));
    state.view.x.0 + (state.view.x.1 - state.view.x.0) * t
}

impl<M: 'static> ChartWidget<M> {
    /// Tells the app where the pointer is, in data, so linked charts can follow.
    fn tell(&self, cx: &mut EventCx<M>, state: &ChartState, px: Option<f32>) {
        if let Some(f) = &self.on_hover {
            let x = px.map(|px| {
                let plot = layout(
                    cx.env.fonts(),
                    &self.chart,
                    cx.content,
                    &state.view,
                    &Self::style(cx.env),
                )
                .plot;
                state_x(state, plot.x, plot.w, px)
            });
            cx.emit(f(x));
        }
    }

    fn key(&self, state: &mut ChartState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed || key.mods.cmd || key.mods.ctrl || key.mods.alt {
            return false;
        }
        let (lo, hi) = state.view.x;
        let span = hi - lo;
        let mid = (lo + hi) / 2.0;
        match key.key {
            Key::Named(NamedKey::ArrowLeft) => {
                state.set_window(&self.chart, (lo - span * 0.1, hi - span * 0.1))
            }
            Key::Named(NamedKey::ArrowRight) => {
                state.set_window(&self.chart, (lo + span * 0.1, hi + span * 0.1))
            }
            Key::Named(NamedKey::Home) => state.set_window(&self.chart, state.extent),
            Key::Character('+' | '=') => state.zoom(&self.chart, mid, 1.4),
            Key::Character('-') => state.zoom(&self.chart, mid, 1.0 / 1.4),
            _ => return false,
        }
        cx.request_frame();
        cx.request_paint();
        true
    }
}

pub struct ChartView<M: 'static> {
    base: Base<M>,
    widget: ChartWidget<M>,
}

builder!(ChartView<M> => ChartWidget<M>);

impl<M: 'static> ChartView<M> {
    /// What assistive technology reads out; by default the layer names.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// Skip the draw-in when the chart first appears.
    pub fn still(mut self) -> Self {
        self.widget.animate = false;
        self
    }

    /// Shows a guide at this x when the pointer is not on the chart. Feed it the value from another
    /// chart's [`on_hover`](Self::on_hover) to make a group follow one pointer.
    pub fn guide(mut self, x: Option<f64>) -> Self {
        self.widget.guide = x;
        self
    }

    /// Sends the data x under the pointer as it moves, and `None` when it leaves.
    pub fn on_hover(mut self, f: impl Fn(Option<f64>) -> M + 'static) -> Self {
        self.widget.on_hover = Some(Rc::new(f));
        self
    }
}

/// A chart that fills the width it is given. Share the data with `Rc` so redrawing the app does not copy it:
/// the chart only looks at it again when the `Rc` is a different one.
pub fn chart<M: 'static>(data: Rc<Chart>) -> ChartView<M> {
    let view = ChartView {
        base: Base::new(),
        widget: ChartWidget {
            chart: data,
            label: String::new(),
            animate: true,
            guide: None,
            on_hover: None,
        },
    };
    view.focus_ring(true).cursor(CursorIcon::Crosshair)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Chart {
        let pts: Vec<(f64, f64)> = (0..100)
            .map(|i| (f64::from(i), 10.0 + f64::from(i % 10)))
            .collect();
        Chart::new().line("a", &pts)
    }

    #[test]
    fn a_new_chart_shows_everything_and_starts_hidden_to_draw_in() {
        let s = ChartState::new(&data(), true);
        assert_eq!(s.view.x, (0.0, 99.0));
        assert_eq!(s.reveal, 0.0);
        assert!(!s.settled());
        assert_eq!(ChartState::new(&data(), false).reveal, 1.0);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_still() {
        let c = data();
        let mut s = ChartState::new(&c, false);
        s.zoom(&c, 25.0, 2.0);
        let (lo, hi) = s.view.x;
        assert!(((hi - lo) - 49.5).abs() < 1e-9);
        let t = (25.0 - lo) / (hi - lo);
        assert!(
            (t - 25.0 / 99.0).abs() < 1e-9,
            "25 stays at the same place across the plot: {t}"
        );
    }

    #[test]
    fn the_window_stays_inside_the_data_and_cannot_zoom_past_a_few_steps() {
        let c = data();
        let mut s = ChartState::new(&c, false);
        s.zoom(&c, 50.0, 0.1);
        assert_eq!(s.view.x, (0.0, 99.0), "zooming out stops at everything");
        s.set_window(&c, (-30.0, 20.0));
        assert_eq!(
            s.view.x,
            (0.0, 50.0),
            "panning past the start stops at the start, keeping the width"
        );
        s.set_window(&c, (90.0, 140.0));
        assert_eq!(s.view.x, (49.0, 99.0));
        for _ in 0..40 {
            s.zoom(&c, 50.0, 3.0);
        }
        assert!(
            (s.view.x.1 - s.view.x.0 - 4.0).abs() < 1e-9,
            "four steps wide at the most: {:?}",
            s.view.x
        );
    }

    #[test]
    fn the_vertical_range_follows_the_window_and_glides_there() {
        let mut pts: Vec<(f64, f64)> = (0..100).map(|i| (f64::from(i), 10.0)).collect();
        pts[90].1 = 500.0;
        let c = Chart::new().line("a", &pts);
        let mut s = ChartState::new(&c, false);
        let tall = s.view.y.1;
        s.set_window(&c, (0.0, 30.0));
        assert!(
            s.target.y.1 < 50.0 && s.view.y.1 == tall,
            "the target moves at once, the picture does not"
        );
        let mut t = 0;
        while !s.settled() && t < 600 {
            let k = 1.0 - (-(1.0f64 / 60.0) * SETTLE).exp();
            s.view.y = glide(s.view.y, s.target.y, k);
            s.view.strip = glide(s.view.strip, s.target.strip, k);
            t += 1;
        }
        assert!(s.settled() && t > 5 && t < 200, "settled after {t} frames");
    }
}
