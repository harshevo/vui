//! Progress bars and the spinner: what is shown while something takes time.

use vui_core::{Corners, Fill, Quad, Rect, Size, Spring, SpringValue, Tone, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, AccessRange, Base, Dirty, Env, PaintCx, Role, TickCx, Widget, builder};

use crate::display::icon::{ICON_SIZE, paint_spinner};

const BAR: f32 = 4.0;
const DEFAULT_WIDTH: f32 = 160.0;
/// The moving part of an indeterminate bar, as a share of the bar, and seconds per pass.
const RUNNER: f32 = 0.35;
const PASS: f32 = 1.4;

/// The widget behind [`Progress`]: a track and the part of it that is done.
pub struct ProgressWidget {
    /// 0 to 1, or `None` when nobody knows how long it will take.
    value: Option<f32>,
}

pub struct ProgressState {
    shown: SpringValue,
}

impl<M: 'static> Widget<M> for ProgressWidget {
    type State = ProgressState;

    fn create_state(&self) -> ProgressState {
        ProgressState { shown: SpringValue::new(self.value.unwrap_or(0.0), Spring::SMOOTH) }
    }

    fn update(&self, old: &ProgressWidget, state: &mut ProgressState) -> Dirty {
        if let Some(v) = self.value {
            state.shown.set_target(v);
        }
        if self.value != old.value { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut ProgressState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = match avail.x {
            Avail::Definite(w) => w.min(DEFAULT_WIDTH),
            _ => DEFAULT_WIDTH,
        };
        size(known.x.unwrap_or(width), known.y.unwrap_or(BAR))
    }

    fn paint(&self, state: &ProgressState, cx: &mut PaintCx) {
        let palette = cx.palette().clone();
        let c = cx.content;
        let track = cx.snap(Rect::new(c.x, c.y + (c.h - BAR) / 2.0, c.w, BAR));
        let round = Corners::all(track.h / 2.0);
        cx.quad(Quad::new(track).fill(palette.wash.mul_alpha(cx.alpha)).radii(round));
        let accent = palette.accent.mul_alpha(cx.alpha);
        match self.value {
            Some(_) => {
                let done = state.shown.value().clamp(0.0, 1.0);
                if done > 0.0 {
                    // Never narrower than the bar is tall, or the round ends would turn into a smudge.
                    let w = (track.w * done).max(track.h.min(track.w));
                    cx.quad(Quad::new(Rect::new(track.x, track.y, w, track.h)).fill(accent).radii(round));
                }
            }
            None => {
                // A runner enters from the left, crosses the bar and leaves on the right, slowing at
                // both ends: the progress an eye reads as "working".
                let phase = (cx.time / PASS).fract();
                let eased = phase * phase * (3.0 - 2.0 * phase);
                let w = track.w * RUNNER;
                let x = track.x - w + (track.w + w) * eased;
                cx.clipped(track, |cx| {
                    cx.quad(Quad::new(Rect::new(x, track.y, w, track.h)).fill(accent).radii(round));
                });
            }
        }
    }

    fn tick(&self, state: &mut ProgressState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = cx.step(&mut state.shown, dt);
        moving || self.value.is_none()
    }

    fn animating(&self, state: &ProgressState) -> bool {
        self.value.is_none() || !state.shown.is_settled()
    }

    fn access(&self, _state: &ProgressState, info: &mut AccessInfo) {
        info.role = Some(Role::ProgressBar);
        if let Some(v) = self.value {
            info.range = Some(AccessRange { min: 0.0, max: 1.0, step: 0.01, value: f64::from(v) });
            info.value = Some(format!("{}%", (v * 100.0).round()));
        }
    }
}

/// A bar that fills as work gets done.
pub struct Progress<M: 'static> {
    base: Base<M>,
    widget: ProgressWidget,
}

builder!(Progress<M> => ProgressWidget);

/// A bar `done` of the way along, from 0 to 1. Name it for assistive technology.
pub fn progress<M: 'static>(done: f32) -> Progress<M> {
    Progress { base: Base::new(), widget: ProgressWidget { value: Some(done.clamp(0.0, 1.0)) } }
}

/// A bar for work of unknown length.
pub fn progress_indeterminate<M: 'static>() -> Progress<M> {
    Progress { base: Base::new(), widget: ProgressWidget { value: None } }
}

/// The widget behind [`Spinner`].
pub struct SpinnerWidget {
    color: Fill,
}

impl<M: 'static> Widget<M> for SpinnerWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &SpinnerWidget, _state: &mut ()) -> Dirty {
        if self.color != old.color { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut (), _env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let side = known.x.or(known.y).unwrap_or(ICON_SIZE);
        size(known.x.unwrap_or(side), known.y.unwrap_or(side))
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        paint_spinner(cx, cx.content, self.color);
    }

    fn tick(&self, _state: &mut (), _cx: &mut TickCx, _dt: f32) -> bool {
        true
    }

    fn animating(&self, _state: &()) -> bool {
        true
    }

    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::ProgressBar);
    }
}

/// A ring that turns while something loads.
pub struct Spinner<M: 'static> {
    base: Base<M>,
    widget: SpinnerWidget,
}

builder!(Spinner<M> => SpinnerWidget);

impl<M: 'static> Spinner<M> {
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }
}

/// A spinner, 16 points square unless sized: `spinner().square(24.0).name("Loading")`.
pub fn spinner<M: 'static>() -> Spinner<M> {
    Spinner { base: Base::new(), widget: SpinnerWidget { color: Tone::Text2.into() } }
}
