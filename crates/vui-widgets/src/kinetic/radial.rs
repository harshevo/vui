//! The radial menu: up to eight choices in a ring around the pointer. Every choice is the same short
//! flick away, so hands learn where each one is.

use std::rc::Rc;

use vui_core::{Corners, CursorIcon, Event, Fill, Key, NamedKey, Point, PointerButton, Quad, Rect, Size, Spring, SpringValue, TextRole, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Builder, Dirty, Div, Env, EventCx, Focus, PaintCx, Parent, Role, Styled, TickCx, Widget, WidgetEvent, builder, div};

use super::line;

/// How far the ring reaches from its centre, and where the choices sit on it.
const RADIUS: f32 = 104.0;
const ORBIT: f32 = 68.0;
/// Inside this the pointer is on no choice: a click there closes the menu.
const DEAD: f32 = 22.0;

type Send<M> = Rc<dyn Fn() -> M>;

/// The widget behind [`radial_menu`].
pub struct RadialWidget<M> {
    /// Shown in the middle: what the choices act on, such as the price that was clicked.
    title: Option<String>,
    labels: Vec<String>,
    tones: Vec<Option<Tone>>,
    sends: Vec<Send<M>>,
    close: Send<M>,
}

pub struct RadialState {
    hover: Option<usize>,
    /// 0 to 1 as the ring opens out from the pointer.
    open: SpringValue,
}

impl<M> RadialWidget<M> {
    /// The centre of choice `i`, clockwise from twelve o'clock, `t` of the way out.
    fn spot(&self, centre: Point, i: usize, t: f32) -> Point {
        let n = self.labels.len().max(1) as f32;
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n;
        pt(centre.x + a.cos() * ORBIT * t, centre.y + a.sin() * ORBIT * t)
    }

    /// The choice in the direction of `p`, or none in the middle or well outside the ring.
    fn aim(&self, centre: Point, p: Point) -> Option<usize> {
        let (dx, dy) = (p.x - centre.x, p.y - centre.y);
        let d = (dx * dx + dy * dy).sqrt();
        if d < DEAD || d > RADIUS + 40.0 || self.labels.is_empty() {
            return None;
        }
        let n = self.labels.len() as f32;
        let a = (dy.atan2(dx) + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        Some(((a / (std::f32::consts::TAU / n)).round() as usize) % self.labels.len())
    }
}

fn centre(r: Rect) -> Point {
    pt(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

impl<M: 'static> Widget<M> for RadialWidget<M> {
    type State = RadialState;

    fn create_state(&self) -> RadialState {
        let mut open = SpringValue::new(0.0, Spring::SNAPPY);
        open.set_target(1.0);
        RadialState { hover: None, open }
    }

    fn update(&self, old: &RadialWidget<M>, _state: &mut RadialState) -> Dirty {
        if self.labels != old.labels || self.tones != old.tones || self.title != old.title { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut RadialState, _env: &Env, _known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        size(RADIUS * 2.0, RADIUS * 2.0)
    }

    fn paint(&self, state: &RadialState, cx: &mut PaintCx) {
        let c = centre(cx.rect);
        let t = state.open.value().clamp(0.0, 1.2);
        let r = RADIUS * t.min(1.0);
        if r < 1.0 {
            return;
        }
        // The disc itself is the layer around the widget, a floating surface. A faint wash of the
        // text colour lifts it off a sheet of the same colour (lighter in dark, greyer in light),
        // and a soft hole sits in the middle unless a title does.
        let side = cx.color(Tone::Side);
        let lift = Rect::new(c.x - r, c.y - r, r * 2.0, r * 2.0);
        // A plain quad: the smoothed corners of fill_rounded would square a full circle off.
        let wash = cx.color(Fill::Tone(Tone::Text, 0.05));
        cx.quad(Quad::new(lift).fill(wash).radii(Corners::all(r)));
        match &self.title {
            Some(title) => {
                let layout = line(cx.env, title, TextRole::Small, 600, true);
                let s = layout.size();
                cx.text(layout, pt(c.x - s.w / 2.0, c.y - s.h / 2.0), Fill::Tone(Tone::Text, t.min(1.0)));
            }
            None => {
                let hole = Rect::new(c.x - DEAD, c.y - DEAD, DEAD * 2.0, DEAD * 2.0);
                cx.quad(Quad::new(hole).fill(side).radii(Corners::all(DEAD)));
            }
        }

        for (i, label) in self.labels.iter().enumerate() {
            let at = self.spot(c, i, t);
            let layout = line(cx.env, label, TextRole::Small, 500, false);
            let s = layout.size();
            let on = state.hover == Some(i);
            let tone = self.tones.get(i).copied().flatten();
            let pill = Rect::new(at.x - s.w / 2.0 - 9.0, at.y - 12.0, s.w + 18.0, 24.0);
            if on {
                let fill = tone.map_or(Fill::from(Tone::Wash), |t| Fill::Tone(t, 0.18));
                cx.fill_rounded(pill, Corners::all(12.0), fill);
            }
            let color = match (on, tone) {
                (_, Some(t)) => t,
                (true, None) => Tone::Text,
                (false, None) => Tone::Text2,
            };
            cx.text(layout, pt(at.x - s.w / 2.0, at.y - s.h / 2.0), color);
        }
    }

    fn event(&self, state: &mut RadialState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let c = centre(cx.rect);
        match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                let aim = self.aim(c, *pos);
                if aim != state.hover {
                    state.hover = aim;
                    cx.request_paint();
                }
                true
            }
            WidgetEvent::Input(Event::PointerUp { pos, button: PointerButton::Primary, .. }) => {
                match self.aim(c, *pos) {
                    Some(i) => cx.emit((self.sends[i])()),
                    None => cx.emit((self.close)()),
                }
                true
            }
            WidgetEvent::Input(Event::PointerDown { .. }) => true,
            WidgetEvent::Input(Event::Key(key)) if key.pressed => {
                let n = self.labels.len();
                if n == 0 {
                    return false;
                }
                let now = state.hover.unwrap_or(n - 1);
                match &key.key {
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::Tab) => state.hover = Some((now + 1) % n),
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => state.hover = Some((now + n - 1) % n),
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        if let Some(i) = state.hover {
                            cx.emit((self.sends[i])());
                        }
                    }
                    // 1 to 8 picks the choice by number, clockwise from the top.
                    Key::Character(ch) if ch.is_ascii_digit() => {
                        let i = ch.to_digit(10).unwrap_or(0) as usize;
                        if (1..=n).contains(&i) {
                            cx.emit((self.sends[i - 1])());
                        }
                    }
                    _ => return false,
                }
                cx.request_paint();
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut RadialState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.open, dt)
    }

    fn animating(&self, state: &RadialState) -> bool {
        !state.open.is_settled()
    }

    fn access(&self, state: &RadialState, info: &mut AccessInfo) {
        info.role = Some(Role::Menu);
        info.label = self.title.clone();
        info.value = state.hover.and_then(|i| self.labels.get(i).cloned());
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}

/// The ring as an element, so it can take focus for the arrow and number keys.
struct Ring<M: 'static> {
    base: Base<M>,
    widget: RadialWidget<M>,
}

builder!(Ring<M> => RadialWidget<M>);

/// A ring of `(label, tone, message)` choices centred on `at` (window coordinates), at most eight,
/// clockwise from twelve o'clock, with `title` in the middle. A click on a choice sends its message; a click in the middle, a
/// press outside or Escape sends `on_close`. Put it in the view only while it is open:
/// `menu_at.map(|p| radial_menu(p, choices, Msg::Close))`.
pub fn radial_menu<M: Clone + 'static>(at: Point, title: Option<String>, choices: Vec<(String, Option<Tone>, M)>, on_close: M) -> Div<M> {
    let (mut labels, mut tones, mut sends) = (Vec::new(), Vec::new(), Vec::<Send<M>>::new());
    for (label, tone, msg) in choices.into_iter().take(8) {
        labels.push(label);
        tones.push(tone);
        sends.push(Rc::new(move || msg.clone()));
    }
    let close = on_close.clone();
    let widget = RadialWidget { title, labels, tones, sends, close: Rc::new(move || close.clone()) };
    let ring = Ring { base: Base::new(), widget }.autofocus().name("Choices");
    div()
        .key("vui-radial-menu")
        .fixed()
        .left(at.x - RADIUS)
        .top(at.y - RADIUS)
        .size(RADIUS * 2.0, RADIUS * 2.0)
        .rounded(vui_runtime::Radius::Pill)
        .bg(Tone::Bg)
        .border(1.0, Fill::Tone(Tone::Text, 0.14))
        .shadow(vui_core::Shadow::FLOATING)
        .floating_glass()
        .appear()
        .on_dismiss(on_close)
        .child(ring)
}
