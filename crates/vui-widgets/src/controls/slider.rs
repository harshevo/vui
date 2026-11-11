//! A slider: drag the thumb, click the track, or use the keys.
//!
//! Like every widget here it is controlled: it shows the value the app gives it and sends the value a
//! drag or a key asks for.

use std::rc::Rc;

use vui_core::{Corners, CursorIcon, Event, Key, KeyEvent, NamedKey, Point, PointerButton, Quad, Rect, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, AccessRange, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, Widget,
    WidgetEvent, builder,
};

const THUMB: f32 = 16.0;
const THUMB_BORDER: f32 = 2.0;
const TRACK: f32 = 4.0;
const HEIGHT: f32 = 20.0;
const DEFAULT_WIDTH: f32 = 160.0;
/// A thumb under the pointer, and one being dragged, get a flat halo this much bigger than they are.
const HALO: f32 = 6.0;
/// How far the keyboard focus ring stands off the thumb: the runtime's ring has a gap and a width.
const RING_GROW: f32 = 3.0;
const RING_WIDTH: f32 = 2.0;

type Handler<M> = Rc<dyn Fn(f64) -> M>;

/// The widget behind [`Slider`].
pub struct SliderWidget<M> {
    value: f64,
    min: f64,
    max: f64,
    step: Option<f64>,
    on_change: Option<Handler<M>>,
}

#[derive(Default)]
pub struct SliderState {
    /// While dragging: how far the pointer is from the centre of the thumb, so grabbing the thumb off
    /// centre does not make it jump.
    grab: Option<f32>,
}

impl<M> SliderWidget<M> {
    fn span(&self) -> f64 {
        (self.max - self.min).max(f64::EPSILON)
    }

    fn fraction(&self) -> f32 {
        (((self.value - self.min) / self.span()) as f32).clamp(0.0, 1.0)
    }

    /// What one arrow key press changes: a step, or a hundredth of the range.
    fn small_step(&self) -> f64 {
        self.step.unwrap_or(self.span() / 100.0)
    }

    /// The value nearest `v` that the slider can have.
    fn snapped(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        match self.step {
            Some(step) if step > 0.0 => {
                let snapped = self.min + ((v - self.min) / step).round() * step;
                // Steps like 0.1 do not add up exactly: keep what is left after rounding to 9 places.
                ((snapped * 1e9).round() / 1e9).clamp(self.min, self.max)
            }
            _ => v,
        }
    }

    /// The track the thumb's centre runs along.
    fn run(content: Rect) -> (f32, f32) {
        (content.x + THUMB / 2.0, (content.w - THUMB).max(1.0))
    }

    fn thumb_centre(&self, content: Rect) -> Point {
        let (x0, w) = Self::run(content);
        Point { x: x0 + w * self.fraction(), y: content.y + content.h / 2.0 }
    }

    fn send(&self, cx: &mut EventCx<M>, to: f64) {
        let to = self.snapped(to);
        if to != self.value
            && let Some(on_change) = &self.on_change
        {
            cx.emit(on_change(to));
        }
    }

    fn set_from_pointer(&self, state: &SliderState, cx: &mut EventCx<M>, x: f32) {
        let (x0, w) = Self::run(cx.content);
        let frac = (((x - state.grab.unwrap_or(0.0)) - x0) / w).clamp(0.0, 1.0);
        self.send(cx, self.min + self.span() * f64::from(frac));
    }

    fn key(&self, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed || key.mods.cmd || key.mods.ctrl || key.mods.alt {
            return false;
        }
        let small = self.small_step();
        let large = self.step.map_or(self.span() / 10.0, |s| s * 10.0).max(small);
        let to = match key.key {
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => self.value + small,
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => self.value - small,
            Key::Named(NamedKey::PageUp) => self.value + large,
            Key::Named(NamedKey::PageDown) => self.value - large,
            Key::Named(NamedKey::Home) => self.min,
            Key::Named(NamedKey::End) => self.max,
            _ => return false,
        };
        self.send(cx, to);
        true
    }
}

impl<M: 'static> Widget<M> for SliderWidget<M> {
    type State = SliderState;

    fn create_state(&self) -> SliderState {
        SliderState::default()
    }

    fn update(&self, old: &SliderWidget<M>, _state: &mut SliderState) -> Dirty {
        if self.value != old.value || self.min != old.min || self.max != old.max { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut SliderState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = match avail.x {
            Avail::Definite(w) => w.clamp(THUMB * 2.0, DEFAULT_WIDTH),
            _ => DEFAULT_WIDTH,
        };
        size(known.x.unwrap_or(width), known.y.unwrap_or(HEIGHT))
    }

    fn paint(&self, _state: &SliderState, cx: &mut PaintCx) {
        let palette = cx.palette().clone();
        let c = cx.content;
        let mid = c.y + c.h / 2.0;
        let centre = self.thumb_centre(c);

        let track = cx.snap(Rect::new(c.x, mid - TRACK / 2.0, c.w, TRACK));
        let round = Corners::all(TRACK / 2.0);
        cx.quad(Quad::new(track).fill(palette.track_off.mul_alpha(cx.alpha)).radii(round));
        let filled = Rect::new(track.x, track.y, (centre.x - track.x).max(0.0), track.h);
        cx.quad(Quad::new(filled).fill(palette.accent.mul_alpha(cx.alpha)).radii(round));

        let thumb = cx.snap(Rect::new(centre.x - THUMB / 2.0, centre.y - THUMB / 2.0, THUMB, THUMB));
        let halo = (cx.hover * 0.12).max(cx.press * 0.2);
        if halo > 0.0 {
            let outer = thumb.inflate(HALO / 2.0, HALO / 2.0);
            cx.quad(Quad::new(outer).fill(palette.accent.mul_alpha(halo * cx.alpha)).radii(Corners::all(outer.h / 2.0)));
        }
        let quad = Quad::new(thumb)
            .fill(palette.bg.mul_alpha(cx.alpha))
            .radii(Corners::all(THUMB / 2.0))
            .border(THUMB_BORDER, palette.accent.mul_alpha(cx.alpha));
        cx.quad(quad);
        if cx.focus > 0.01 {
            let ring = thumb.inflate(RING_GROW, RING_GROW);
            let quad = Quad::new(ring)
                .radii(Corners::all(ring.h / 2.0))
                .border(RING_WIDTH, palette.accent.mul_alpha(cx.focus * cx.alpha));
            cx.quad(quad);
        }
    }

    fn event(&self, state: &mut SliderState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                let centre = self.thumb_centre(cx.content);
                let on_thumb = (pos.x - centre.x).abs() <= THUMB / 2.0 + 2.0 && (pos.y - centre.y).abs() <= THUMB / 2.0 + 2.0;
                state.grab = on_thumb.then_some(pos.x - centre.x).or(Some(0.0));
                self.set_from_pointer(state, cx, pos.x);
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.grab.is_some() => {
                self.set_from_pointer(state, cx, pos.x);
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => state.grab.take().is_some(),
            WidgetEvent::Input(Event::Key(key)) => self.key(cx, key),
            WidgetEvent::Action(AccessAction::Increment) => {
                self.send(cx, self.value + self.small_step());
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.send(cx, self.value - self.small_step());
                true
            }
            WidgetEvent::Action(AccessAction::SetNumeric(to)) => {
                self.send(cx, *to);
                true
            }
            _ => false,
        }
    }

    fn access(&self, _state: &SliderState, info: &mut AccessInfo) {
        info.role = Some(Role::Slider);
        info.range = Some(AccessRange { min: self.min, max: self.max, step: self.small_step(), value: self.value });
        info.value = Some(trimmed(self.value));
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A number the way a person reads it: `40`, not `40.000000`, and `0.25`, not `0.2500000001`.
fn trimmed(v: f64) -> String {
    let s = format!("{:.4}", (v * 1e4).round() / 1e4);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A slider over a range. It sends the new value to `on_change` as the thumb moves.
pub struct Slider<M: 'static> {
    base: Base<M>,
    widget: SliderWidget<M>,
}

builder!(Slider<M> => SliderWidget<M>);

impl<M: 'static> Slider<M> {
    /// Moves in whole multiples of `step` from the minimum, and the arrow keys move by one.
    pub fn step(mut self, step: f64) -> Self {
        self.widget.step = Some(step);
        self
    }

    pub fn on_change(mut self, f: impl Fn(f64) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }
}

/// A slider showing `value` between `min` and `max`. Name it for assistive technology:
/// `slider(volume, 0.0, 100.0).name("Volume")`.
pub fn slider<M: 'static>(value: f64, min: f64, max: f64) -> Slider<M> {
    let (min, max) = if min <= max { (min, max) } else { (max, min) };
    let slider = Slider { base: Base::new(), widget: SliderWidget { value: value.clamp(min, max), min, max, step: None, on_change: None } };
    slider.focus_ring(false).cursor(CursorIcon::Pointer)
}
