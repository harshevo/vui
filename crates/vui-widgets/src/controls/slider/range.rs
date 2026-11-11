//! A slider with two thumbs, for a range: a price from and to, a window of time. The thumbs never
//! cross. Tab moves from the low thumb to the high one before it leaves the slider, and Shift-Tab back.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Key, NamedKey, Point, PointerButton, Size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, AccessRange, Base, Builder, Dirty, Env, EventCx, Focus, FocusBy, PaintCx, Role,
    Widget, WidgetEvent, builder,
};

use super::{Track, track_options};

type Handler<M> = Rc<dyn Fn(f64, f64) -> M>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Thumb {
    Low,
    High,
}

/// The widget behind [`RangeSlider`].
pub struct RangeSliderWidget<M> {
    low: f64,
    high: f64,
    /// The least the two may be apart.
    gap: f64,
    track: Track,
    on_change: Option<Handler<M>>,
}

/// Which thumb the keys move, and the one being dragged with how far off centre it was taken.
pub struct RangeState {
    active: Thumb,
    grab: Option<(Thumb, f32)>,
}

impl<M> RangeSliderWidget<M> {
    fn value(&self, thumb: Thumb) -> f64 {
        match thumb {
            Thumb::Low => self.low,
            Thumb::High => self.high,
        }
    }

    fn centre(&self, content: vui_core::Rect, thumb: Thumb) -> Point {
        self.track.at(content, self.track.fraction(self.value(thumb)))
    }

    /// Moves one thumb, stopping it at the other.
    fn send(&self, cx: &mut EventCx<M>, thumb: Thumb, to: f64) {
        let to = self.track.snapped(to);
        let (low, high) = match thumb {
            Thumb::Low => (to.min(self.high - self.gap).max(self.track.min), self.high),
            Thumb::High => (self.low, to.max(self.low + self.gap).min(self.track.max)),
        };
        if (low, high) != (self.low, self.high)
            && let Some(on_change) = &self.on_change
        {
            cx.emit(on_change(low, high));
        }
    }

    /// The thumb a press at `p` takes: the nearer one, or when they sit together, the one on the side
    /// the pointer is.
    fn pick(&self, content: vui_core::Rect, p: Point) -> Thumb {
        let (lo, hi) = (self.centre(content, Thumb::Low), self.centre(content, Thumb::High));
        let (dl, dh) = (self.track.along(lo, p), self.track.along(hi, p));
        let beyond = if self.track.vertical { dh < 0.0 } else { dh > 0.0 };
        if (dl.abs() - dh.abs()).abs() < 0.5 {
            if beyond { Thumb::High } else { Thumb::Low }
        } else if dl.abs() < dh.abs() {
            Thumb::Low
        } else {
            Thumb::High
        }
    }
}

impl<M: 'static> Widget<M> for RangeSliderWidget<M> {
    type State = RangeState;

    fn create_state(&self) -> RangeState {
        RangeState { active: Thumb::Low, grab: None }
    }

    fn update(&self, old: &RangeSliderWidget<M>, _state: &mut RangeState) -> Dirty {
        if self.track.vertical != old.track.vertical {
            Dirty::Layout
        } else if (self.low, self.high) != (old.low, old.high) || self.track != old.track {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, _state: &mut RangeState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        self.track.measure(known, avail)
    }

    fn paint(&self, state: &RangeState, cx: &mut PaintCx) {
        let (lo, hi) = (self.track.fraction(self.low), self.track.fraction(self.high));
        self.track.paint_track(cx, lo, hi);
        // The active thumb goes last, so it is on top when the two meet.
        let order = if state.active == Thumb::Low { [Thumb::High, Thumb::Low] } else { [Thumb::Low, Thumb::High] };
        for thumb in order {
            let lit = thumb == state.active;
            let halo = if lit { (cx.hover * 0.12).max(cx.press * 0.2) } else { 0.0 };
            let focus = if lit { cx.focus } else { 0.0 };
            Track::paint_thumb(cx, self.centre(cx.content, thumb), halo, focus);
        }
    }

    fn paint_over(&self, state: &RangeState, cx: &mut PaintCx) {
        let shown = cx.press.max(cx.hover).max(cx.focus);
        let thumb = state.active;
        self.track.paint_bubble(cx, self.centre(cx.content, thumb), self.value(thumb), shown);
    }

    fn event(&self, state: &mut RangeState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                let thumb = self.pick(cx.content, *pos);
                let centre = self.centre(cx.content, thumb);
                let grab = if Track::on_thumb(centre, *pos) { self.track.along(centre, *pos) } else { 0.0 };
                state.active = thumb;
                state.grab = Some((thumb, grab));
                self.send(cx, thumb, self.track.value_at(cx.content, *pos, grab));
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.grab.is_some() => {
                if let Some((thumb, grab)) = state.grab {
                    self.send(cx, thumb, self.track.value_at(cx.content, *pos, grab));
                }
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => state.grab.take().is_some(),
            WidgetEvent::FocusChanged { focused: true, by: FocusBy::Tab, .. } => {
                state.active = if cx.mods.shift { Thumb::High } else { Thumb::Low };
                cx.request_paint();
                false
            }
            WidgetEvent::Input(Event::Key(key)) if key.pressed && key.key == Key::Named(NamedKey::Tab) && !key.mods.cmd && !key.mods.ctrl && !key.mods.alt => {
                let to = if key.mods.shift { Thumb::Low } else { Thumb::High };
                if state.active == to {
                    return false;
                }
                state.active = to;
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Key(key)) => match self.track.key(self.value(state.active), key) {
                Some(to) => {
                    self.send(cx, state.active, to);
                    true
                }
                None => false,
            },
            WidgetEvent::Action(AccessAction::Increment) => {
                self.send(cx, state.active, self.value(state.active) + self.track.small_step());
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.send(cx, state.active, self.value(state.active) - self.track.small_step());
                true
            }
            WidgetEvent::Action(AccessAction::SetNumeric(to)) => {
                self.send(cx, state.active, *to);
                true
            }
            _ => false,
        }
    }

    fn access(&self, state: &RangeState, info: &mut AccessInfo) {
        info.role = Some(Role::Slider);
        let t = &self.track;
        info.range = Some(AccessRange { min: t.min, max: t.max, step: t.small_step(), value: self.value(state.active) });
        info.value = Some(format!("{} to {}", t.text(self.low), t.text(self.high)));
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A slider with two thumbs. It sends both values to `on_change` as either moves.
pub struct RangeSlider<M: 'static> {
    base: Base<M>,
    widget: RangeSliderWidget<M>,
}

builder!(RangeSlider<M> => RangeSliderWidget<M>);
track_options!(RangeSlider);

impl<M: 'static> RangeSlider<M> {
    /// The least the two values may be apart; 0 by default, so they can meet.
    pub fn gap(mut self, gap: f64) -> Self {
        self.widget.gap = gap.max(0.0);
        self
    }

    pub fn on_change(mut self, f: impl Fn(f64, f64) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }
}

/// A slider showing `low` to `high` between `min` and `max`:
/// `range_slider(lo, hi, 0.0, 500.0).step(10.0).name("Price").on_change(Msg::Price)`.
pub fn range_slider<M: 'static>(low: f64, high: f64, min: f64, max: f64) -> RangeSlider<M> {
    let track = Track::new(min, max);
    let (low, high) = (low.min(high).clamp(track.min, track.max), high.max(low).clamp(track.min, track.max));
    let widget = RangeSliderWidget { low, high, gap: 0.0, track, on_change: None };
    RangeSlider { base: Base::new(), widget }.focus_ring(false).cursor(CursorIcon::Pointer)
}
