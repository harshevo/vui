//! The colour picker's pads: a square that picks two values by where it is pressed and dragged, and
//! a strip that picks one. Both move with the arrow keys too.

use std::rc::Rc;

use vui_core::{Color, Corners, Event, Key, KeyEvent, NamedKey, PointerButton, Quad, Rect, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, Styled, Widget, WidgetEvent, builder,
};

type Handler<M> = Rc<dyn Fn(f32, f32) -> M>;

/// What a pad draws under its thumb.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum Ground {
    /// Nothing: the element's own decoration shows through (the saturation and value square).
    Plain,
    /// Every hue, left to right.
    Hues,
}

/// The widget behind [`Pad`].
pub struct PadWidget<M> {
    /// Where the thumb is, 0 to 1 across and down.
    x: f32,
    y: f32,
    /// Moves on both axes, or only across.
    both: bool,
    ground: Ground,
    on_change: Option<Handler<M>>,
}

#[derive(Default)]
pub struct PadState {
    dragging: bool,
}

impl<M> PadWidget<M> {
    fn send(&self, cx: &mut EventCx<M>, x: f32, y: f32) {
        if let Some(f) = &self.on_change {
            cx.emit(f(x.clamp(0.0, 1.0), if self.both { y.clamp(0.0, 1.0) } else { self.y }));
        }
    }

    fn at(&self, cx: &mut EventCx<M>, pos: vui_core::Point) {
        let c = cx.content;
        self.send(cx, (pos.x - c.x) / c.w.max(1.0), (pos.y - c.y) / c.h.max(1.0));
    }

    fn key(&self, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed {
            return false;
        }
        let step = if key.mods.shift { 0.1 } else { 0.01 };
        let (dx, dy) = match key.key {
            Key::Named(NamedKey::ArrowLeft) => (-step, 0.0),
            Key::Named(NamedKey::ArrowRight) => (step, 0.0),
            Key::Named(NamedKey::ArrowUp) if self.both => (0.0, -step),
            Key::Named(NamedKey::ArrowDown) if self.both => (0.0, step),
            _ => return false,
        };
        self.send(cx, self.x + dx, self.y + dy);
        true
    }
}

impl<M: 'static> Widget<M> for PadWidget<M> {
    type State = PadState;

    fn create_state(&self) -> PadState {
        PadState::default()
    }

    fn update(&self, old: &PadWidget<M>, _state: &mut PadState) -> Dirty {
        if (self.x, self.y, self.ground) == (old.x, old.y, old.ground) { Dirty::None } else { Dirty::Paint }
    }

    fn measure(&self, _state: &mut PadState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let w = known.x.or(avail.x.definite()).unwrap_or(200.0);
        size(w, known.y.unwrap_or(if self.both { 160.0 } else { 14.0 }))
    }

    fn paint(&self, _state: &PadState, cx: &mut PaintCx) {
        let c = cx.content;
        if self.ground == Ground::Hues {
            const BANDS: usize = 96;
            let w = c.w / BANDS as f32;
            for i in 0..BANDS {
                let hue = (i as f32 + 0.5) / BANDS as f32 * 360.0;
                let band = Rect::new(c.x + i as f32 * w, c.y, w + 0.5, c.h);
                let end = c.h / 2.0;
                let radii = Corners { tl: if i == 0 { end } else { 0.0 }, bl: if i == 0 { end } else { 0.0 }, tr: if i == BANDS - 1 { end } else { 0.0 }, br: if i == BANDS - 1 { end } else { 0.0 } };
                cx.quad(Quad::new(band).radii(radii).fill(Color::from_hsv(hue, 1.0, 1.0).mul_alpha(cx.alpha)));
            }
        }
        let r = 7.0;
        let at = vui_core::pt(c.x + self.x * c.w, c.y + if self.both { self.y * c.h } else { c.h / 2.0 });
        let ring = |grow: f32| Rect::new(at.x - r - grow, at.y - r - grow, (r + grow) * 2.0, (r + grow) * 2.0);
        cx.quad(Quad::new(ring(1.0)).radii(r + 1.0).border(1.0, Color::BLACK.with_alpha(0.35 * cx.alpha)));
        cx.quad(Quad::new(ring(0.0)).radii(r).border(2.5, Color::WHITE.mul_alpha(cx.alpha)));
    }

    fn event(&self, state: &mut PadState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.dragging = true;
                cx.capture_pointer();
                self.at(cx, *pos);
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.dragging => {
                self.at(cx, *pos);
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) if state.dragging => {
                state.dragging = false;
                cx.release_pointer();
                true
            }
            WidgetEvent::Input(Event::Key(key)) => self.key(cx, key),
            WidgetEvent::Action(AccessAction::Increment) => {
                self.send(cx, self.x + 0.05, self.y);
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.send(cx, self.x - 0.05, self.y);
                true
            }
            _ => false,
        }
    }

    fn access(&self, _state: &PadState, info: &mut AccessInfo) {
        info.role = Some(Role::Slider);
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A pad: see [`PadWidget`].
pub struct Pad<M: 'static> {
    base: Base<M>,
    widget: PadWidget<M>,
}

builder!(Pad<M> => PadWidget<M>);

impl<M: 'static> Pad<M> {
    /// Sent with where the thumb should go, 0 to 1 across and down.
    pub fn on_change(mut self, f: impl Fn(f32, f32) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }
}

pub(super) fn pad<M: 'static>(x: f32, y: f32, both: bool, ground: Ground) -> Pad<M> {
    Pad { base: Base::new(), widget: PadWidget { x, y, both, ground, on_change: None } }.cursor(vui_core::CursorIcon::Pointer).fill_width()
}
