//! A scrubbable value: a number that is its own slider. Drag sideways to change it (Shift for ten
//! steps at a time, Option for a tenth of one), click to type it, arrow keys to nudge it.

use std::rc::Rc;

use vui_core::{Corners, CursorIcon, Event, Fill, Key, NamedKey, PointerButton, Quad, Rect, Size, TextRole, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, AccessRange, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Radius, Role, Styled, Widget,
    WidgetEvent, builder,
};

use super::line;

const PAD: f32 = 8.0;
/// Between the name and the value.
const GAP: f32 = 6.0;
/// The pointer moves this far sideways for one step.
const PX_PER_STEP: f32 = 4.0;
/// Below this the press is a click, which starts typing.
const SLOP: f32 = 3.0;

/// The widget behind [`Scrub`].
pub struct ScrubWidget<M> {
    value: Option<f64>,
    /// Where a drag starts from while there is no value: the last price for an empty stop.
    anchor: f64,
    step: f64,
    min: f64,
    max: f64,
    decimals: usize,
    name: Option<String>,
    unit: Option<String>,
    placeholder: String,
    on_change: Option<Rc<dyn Fn(f64) -> M>>,
    on_clear: Option<Rc<dyn Fn() -> M>>,
    /// How the value is written when not being typed: grouping, a currency sign.
    format: Option<Rc<dyn Fn(f64) -> String>>,
    /// The value centred in what is left of the field after its name, rather than at the right.
    centered: bool,
}

pub struct ScrubState {
    drag: Option<Drag>,
    /// What is typed so far while typing.
    edit: Option<String>,
    /// Seconds left of the head shake for typing that is not a number.
    shake: f32,
    /// Typing has just started on the old value, which shows selected: the first key replaces it.
    fresh: bool,
}

struct Drag {
    x: f32,
    from: f64,
    moved: bool,
}

impl<M> ScrubWidget<M> {
    fn text(&self, v: f64) -> String {
        let s = match &self.format {
            Some(f) => f(v),
            None => format!("{:.*}", self.decimals, v),
        };
        match &self.unit {
            Some(u) => format!("{s}{u}"),
            None => s,
        }
    }

    fn shown(&self, state: &ScrubState) -> (String, bool) {
        match (&state.edit, self.value) {
            (Some(typed), _) => (typed.clone(), true),
            (None, Some(v)) => (self.text(v), true),
            (None, None) => (self.placeholder.clone(), false),
        }
    }

    /// On the step grid and inside the range.
    fn tidy(&self, v: f64) -> f64 {
        let v = if self.step > 0.0 { (v / self.step).round() * self.step } else { v };
        let p = 10f64.powi(self.decimals as i32);
        ((v * p).round() / p).clamp(self.min, self.max)
    }

    fn send(&self, cx: &mut EventCx<M>, v: f64) {
        let v = self.tidy(v);
        if Some(v) != self.value
            && let Some(f) = &self.on_change
        {
            cx.emit(f(v));
        }
    }

    fn nudge(&self, cx: &mut EventCx<M>, steps: f64) {
        self.send(cx, self.value.unwrap_or(self.anchor) + steps * self.step);
    }

    fn commit(&self, cx: &mut EventCx<M>, state: &mut ScrubState) {
        let Some(typed) = state.edit.take() else { return };
        let typed = typed.trim().replace(',', "");
        if typed.is_empty() {
            if self.value.is_some()
                && let Some(clear) = &self.on_clear
            {
                cx.emit(clear());
            }
        } else if let Ok(v) = typed.parse::<f64>() {
            self.send(cx, v);
        } else {
            // Not a number: keep it to fix, and shake.
            state.edit = Some(typed);
            state.shake = super::SHAKE;
            cx.request_frame();
        }
        cx.request_paint();
    }

    fn typing(&self, state: &mut ScrubState, cx: &mut EventCx<M>, key: &vui_core::KeyEvent) -> bool {
        let Some(edit) = state.edit.as_mut() else { return false };
        match &key.key {
            Key::Named(NamedKey::Enter) => self.commit(cx, state),
            Key::Named(NamedKey::Escape) => {
                state.edit = None;
                cx.request_paint();
            }
            Key::Named(NamedKey::Backspace) => {
                if state.fresh {
                    edit.clear();
                } else {
                    edit.pop();
                }
                state.fresh = false;
                cx.request_paint();
            }
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                let up = key.key == Key::Named(NamedKey::ArrowUp);
                let now = edit.trim().parse::<f64>().unwrap_or(self.value.unwrap_or(self.anchor));
                *edit = format!("{:.*}", self.decimals, self.tidy(now + if up { self.step } else { -self.step }));
                state.fresh = false;
                cx.request_paint();
            }
            _ if key.mods.shortcut() => return false,
            _ => {
                let Some(text) = &key.text else { return false };
                let ok: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                if ok.is_empty() {
                    return false;
                }
                if state.fresh {
                    edit.clear();
                    state.fresh = false;
                }
                edit.push_str(&ok);
                cx.request_paint();
            }
        }
        true
    }
}

impl<M: 'static> Widget<M> for ScrubWidget<M> {
    type State = ScrubState;

    fn create_state(&self) -> ScrubState {
        ScrubState { drag: None, edit: None, shake: 0.0, fresh: false }
    }

    fn update(&self, old: &ScrubWidget<M>, _state: &mut ScrubState) -> Dirty {
        if self.name != old.name || self.decimals != old.decimals || self.unit != old.unit {
            Dirty::Layout
        } else if self.value != old.value || self.placeholder != old.placeholder {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut ScrubState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let (text, _) = self.shown(state);
        let value = line(env, &text, TextRole::Small, 500, true).size().w.max(40.0);
        let name = self.name.as_ref().map_or(0.0, |n| line(env, n, TextRole::Small, 400, false).size().w + GAP);
        size(known.x.unwrap_or(name + value), known.y.unwrap_or(28.0))
    }

    fn paint(&self, state: &ScrubState, cx: &mut PaintCx) {
        let mut content = cx.content;
        content.x += super::shake_offset(state.shake, cx.env.prefs.reduce_motion);
        let mid = content.y + content.h / 2.0;
        let mut named = content.x;
        if let Some(name) = &self.name {
            let layout = line(cx.env, name, TextRole::Small, 400, false);
            let h = layout.size().h;
            named += layout.size().w;
            cx.text(layout, pt(content.x, mid - h / 2.0), Tone::Text3);
        }
        let (text, real) = self.shown(state);
        let layout = line(cx.env, &text, TextRole::Small, 500, true);
        let s = layout.size();
        let right = content.x + content.w;
        let x = if self.centered { (named + right - s.w) / 2.0 } else { right - s.w };
        cx.text(layout, pt(x, mid - s.h / 2.0), if real { Tone::Text } else { Tone::Text3 });

        if state.edit.is_some() && state.fresh {
            let sel = Rect::new(x - 2.0, mid - s.h / 2.0, s.w + 4.0, s.h);
            let wash = cx.color(Fill::Tone(Tone::Accent, 0.22));
            cx.quad(Quad::new(cx.snap(sel)).fill(wash).radii(Corners::all(3.0)));
        }
        if state.edit.is_some() {
            // Typing: a caret after the text and a thin accent ring.
            let caret = Rect::new(x + s.w + 1.0, mid - s.h / 2.0, 1.5, s.h);
            let accent = cx.color(Tone::Accent);
            cx.quad(Quad::new(cx.snap(caret)).fill(accent));
            let r = cx.radii;
            cx.quad(Quad::new(cx.rect).radii(r).border(1.0, accent));
        } else if cx.hover > 0.01 {
            // Chevrons either side of the value say it moves sideways; a dotted line under it says
            // it can be dragged.
            let fill = Fill::Tone(Tone::Text3, cx.hover);
            let weight = vui_icons::Weight::Regular;
            // Small enough to sit in the padding on the right; the left one only where the label
            // leaves room for it.
            let (c, half) = (7.0, 3.5);
            if x - c - 4.0 >= named + 4.0 {
                crate::display::icon::paint_icon(cx, vui_icons::set::CHEVRON_LEFT, Rect::new(x - c - 4.0, mid - half, c, c), fill, weight);
            }
            crate::display::icon::paint_icon(cx, vui_icons::set::CHEVRON_RIGHT, Rect::new(content.x + content.w + 2.0, mid - half, c, c), fill, weight);
            if real {
                let color = cx.color(Fill::Tone(Tone::Text3, 0.8 * cx.hover));
                let y = mid + s.h / 2.0 + 1.0;
                let mut dx = 0.0;
                while dx < s.w {
                    cx.quad(Quad::new(cx.snap(Rect::new(x + dx, y, 2.0, 1.0))).fill(color).radii(Corners::all(0.5)));
                    dx += 4.0;
                }
            }
        }
    }

    fn event(&self, state: &mut ScrubState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                if state.edit.is_none() {
                    state.drag = Some(Drag { x: pos.x, from: self.value.unwrap_or(self.anchor), moved: false });
                    cx.capture_pointer();
                    cx.focus();
                }
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, mods }) => {
                let Some(drag) = state.drag.as_mut() else { return false };
                let dx = pos.x - drag.x;
                if !drag.moved && dx.abs() < SLOP {
                    return true;
                }
                drag.moved = true;
                let scale = if mods.shift {
                    10.0
                } else if mods.alt {
                    0.1
                } else {
                    1.0
                };
                let steps = (dx / PX_PER_STEP).trunc() as f64;
                let from = drag.from;
                self.send(cx, from + steps * self.step * scale);
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => {
                let Some(drag) = state.drag.take() else { return false };
                cx.release_pointer();
                if !drag.moved {
                    state.edit = Some(self.value.map(|v| format!("{:.*}", self.decimals, v)).unwrap_or_default());
                    state.fresh = true;
                    cx.request_paint();
                }
                true
            }
            WidgetEvent::Input(Event::Key(key)) if key.pressed => {
                if state.edit.is_some() {
                    return self.typing(state, cx, key);
                }
                match &key.key {
                    Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowRight) => {
                        self.nudge(cx, if key.mods.shift { 10.0 } else { 1.0 });
                        true
                    }
                    Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowLeft) => {
                        self.nudge(cx, if key.mods.shift { -10.0 } else { -1.0 });
                        true
                    }
                    Key::Named(NamedKey::Enter) => {
                        state.edit = Some(self.value.map(|v| format!("{:.*}", self.decimals, v)).unwrap_or_default());
                        state.fresh = true;
                        cx.request_paint();
                        true
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if self.value.is_some()
                            && let Some(clear) = &self.on_clear
                        {
                            cx.emit(clear());
                        }
                        true
                    }
                    _ if key.mods.shortcut() => false,
                    // A digit starts typing a new value straight away.
                    _ => match &key.text {
                        Some(t) if t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && !t.is_empty() => {
                            state.edit = Some(t.clone());
                            state.fresh = false;
                            cx.request_paint();
                            true
                        }
                        _ => false,
                    },
                }
            }
            WidgetEvent::FocusChanged { focused: false, .. } => {
                // Leaving keeps a number that was typed and drops one that wasn't.
                self.commit(cx, state);
                state.edit = None;
                false
            }
            WidgetEvent::Action(AccessAction::Increment) => {
                self.nudge(cx, 1.0);
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.nudge(cx, -1.0);
                true
            }
            WidgetEvent::Action(AccessAction::SetNumeric(v)) => {
                self.send(cx, *v);
                true
            }
            WidgetEvent::Action(AccessAction::SetValue(text)) => {
                if let Ok(v) = text.trim().parse::<f64>() {
                    self.send(cx, v);
                }
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut ScrubState, cx: &mut vui_runtime::TickCx, dt: f32) -> bool {
        state.shake = (state.shake - dt).max(0.0);
        cx.request_paint();
        state.shake > 0.0
    }

    fn animating(&self, state: &ScrubState) -> bool {
        state.shake > 0.0
    }

    fn access(&self, state: &ScrubState, info: &mut AccessInfo) {
        info.role = Some(Role::Slider);
        info.label = self.name.clone();
        info.value = Some(self.shown(state).0);
        info.range = Some(AccessRange { min: self.min, max: self.max, step: self.step, value: self.value.unwrap_or(self.anchor) });
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::ResizeHorizontal)
    }
}

/// A number you drag to change and click to type.
pub struct Scrub<M: 'static> {
    base: Base<M>,
    widget: ScrubWidget<M>,
}

builder!(Scrub<M> => ScrubWidget<M>);

impl<M: 'static> Scrub<M> {
    /// How much one step is. Decimals follow it unless [`decimals`](Self::decimals) says otherwise.
    pub fn step(mut self, step: f64) -> Self {
        self.widget.step = step;
        self.widget.decimals = decimals_of(step);
        self
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.widget.decimals = decimals;
        self
    }

    /// The value in the middle of the field rather than at its right, as a stepper's count is.
    pub fn centered(mut self) -> Self {
        self.widget.centered = true;
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.widget.min = min;
        self.widget.max = max;
        self
    }

    /// A small grey name before the value, inside the control: "Qty 1".
    pub fn label(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.widget.name = (!name.is_empty()).then_some(name);
        self
    }

    /// Written after the number: "%", " min".
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.widget.unit = Some(unit.into());
        self
    }

    /// Shown while there is no value.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.widget.placeholder = text.into();
        self
    }

    /// Where a drag starts while there is no value.
    pub fn anchor(mut self, anchor: f64) -> Self {
        self.widget.anchor = anchor;
        self
    }

    pub fn on_change(mut self, f: impl Fn(f64) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }

    /// How the value is written while it is not being typed: `|v| format!("₹{v:.2}")`.
    pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> Self {
        self.widget.format = Some(Rc::new(f));
        self
    }

    /// Sent when the value is typed away or Backspace is pressed on it. Without it the value stays.
    pub fn on_clear(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.widget.on_clear = Some(Rc::new(f));
        self
    }
}

/// How many decimals a step needs: 0.05 → 2.
fn decimals_of(step: f64) -> usize {
    (0..6).find(|d| ((step * 10f64.powi(*d)).round() - step * 10f64.powi(*d)).abs() < 1e-9).unwrap_or(6) as usize
}

/// `value`, or none yet, as a number to drag or type: `scrub(Some(qty)).label("Qty").step(1.0)
/// .on_change(Msg::Qty)`.
pub fn scrub<M: 'static>(value: Option<f64>) -> Scrub<M> {
    let widget = ScrubWidget {
        value,
        anchor: value.unwrap_or(0.0),
        step: 1.0,
        min: f64::MIN,
        max: f64::MAX,
        decimals: 0,
        name: None,
        unit: None,
        placeholder: "—".into(),
        on_change: None,
        on_clear: None,
        format: None,
        centered: false,
    };
    Scrub { base: Base::new(), widget }.rounded(Radius::Control).bg(Fill::Tone(Tone::Text, 0.04)).hover_bg(Tone::Hover).padding_x(PAD)
}

#[cfg(test)]
mod tests {
    use super::decimals_of;

    #[test]
    fn decimals_follow_the_step() {
        assert_eq!(decimals_of(1.0), 0);
        assert_eq!(decimals_of(0.05), 2);
        assert_eq!(decimals_of(0.5), 1);
        assert_eq!(decimals_of(25.0), 0);
    }
}
