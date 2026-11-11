//! A slider with one thumb.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, PointerButton, Size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, AccessRange, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, Widget,
    WidgetEvent, builder,
};

use super::{Track, track_options};

type Handler<M> = Rc<dyn Fn(f64) -> M>;

/// The widget behind [`Slider`].
pub struct SliderWidget<M> {
    value: f64,
    track: Track,
    on_change: Option<Handler<M>>,
}

#[derive(Default)]
pub struct SliderState {
    /// While dragging: how far the pointer is from the centre of the thumb, along the track, so
    /// grabbing the thumb off centre does not make it jump.
    grab: Option<f32>,
}

impl<M> SliderWidget<M> {
    fn send(&self, cx: &mut EventCx<M>, to: f64) {
        let to = self.track.snapped(to);
        if to != self.value
            && let Some(on_change) = &self.on_change
        {
            cx.emit(on_change(to));
        }
    }
}

impl<M: 'static> Widget<M> for SliderWidget<M> {
    type State = SliderState;

    fn create_state(&self) -> SliderState {
        SliderState::default()
    }

    fn update(&self, old: &SliderWidget<M>, _state: &mut SliderState) -> Dirty {
        if self.track.vertical != old.track.vertical {
            Dirty::Layout
        } else if self.value != old.value || self.track != old.track {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, _state: &mut SliderState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        self.track.measure(known, avail)
    }

    fn paint(&self, _state: &SliderState, cx: &mut PaintCx) {
        let f = self.track.fraction(self.value);
        self.track.paint_track(cx, 0.0, f);
        let centre = self.track.at(cx.content, f);
        let (halo, focus) = ((cx.hover * 0.12).max(cx.press * 0.2), cx.focus);
        Track::paint_thumb(cx, centre, halo, focus);
    }

    fn paint_over(&self, _state: &SliderState, cx: &mut PaintCx) {
        let centre = self.track.at(cx.content, self.track.fraction(self.value));
        let shown = cx.press.max(cx.hover).max(cx.focus);
        self.track.paint_bubble(cx, centre, self.value, shown);
    }

    fn event(&self, state: &mut SliderState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                let centre = self.track.at(cx.content, self.track.fraction(self.value));
                let grab = if Track::on_thumb(centre, *pos) { self.track.along(centre, *pos) } else { 0.0 };
                state.grab = Some(grab);
                self.send(cx, self.track.value_at(cx.content, *pos, grab));
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.grab.is_some() => {
                self.send(cx, self.track.value_at(cx.content, *pos, state.grab.unwrap_or(0.0)));
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => state.grab.take().is_some(),
            WidgetEvent::Input(Event::Key(key)) => match self.track.key(self.value, key) {
                Some(to) => {
                    self.send(cx, to);
                    true
                }
                None => false,
            },
            WidgetEvent::Action(AccessAction::Increment) => {
                self.send(cx, self.value + self.track.small_step());
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.send(cx, self.value - self.track.small_step());
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
        let t = &self.track;
        info.range = Some(AccessRange { min: t.min, max: t.max, step: t.small_step(), value: self.value });
        info.value = Some(t.text(self.value));
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A slider over a range. It sends the new value to `on_change` as the thumb moves.
pub struct Slider<M: 'static> {
    base: Base<M>,
    widget: SliderWidget<M>,
}

builder!(Slider<M> => SliderWidget<M>);
track_options!(Slider);

impl<M: 'static> Slider<M> {
    pub fn on_change(mut self, f: impl Fn(f64) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }
}

/// A slider showing `value` between `min` and `max`. Name it for assistive technology:
/// `slider(volume, 0.0, 100.0).name("Volume")`.
pub fn slider<M: 'static>(value: f64, min: f64, max: f64) -> Slider<M> {
    let track = Track::new(min, max);
    let value = value.clamp(track.min, track.max);
    let slider = Slider { base: Base::new(), widget: SliderWidget { value, track, on_change: None } };
    slider.focus_ring(false).cursor(CursorIcon::Pointer)
}
