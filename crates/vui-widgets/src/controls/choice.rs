//! The small controls that hold a choice: the switch, the checkbox and the radio button.
//!
//! All three are one widget that draws a box, a switch track or a ring and then an optional label,
//! and that eases between its two states with a spring. Flipping one is the app's business: each takes
//! the state it is in and a handler that gets the state a click asks for.

use std::rc::Rc;

use vui_core::{Color, Corners, Fill, Quad, Rect, Size, Spring, SpringValue, TextLayout, TextStyle, TextWrap, Tone, pt, size};
use vui_icons::{Weight, set};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AUTO, AccessInfo, Align, Base, Builder, Dirty, Div, Env, EventCx, Focus, FocusBy, FocusGroup, PaintCx, Parent, Radius, Role, Styled,
    TickCx, Widget, WidgetEvent, builder, column,
};

use crate::display::icon::paint_icon;

/// Space between the control and its label.
const GAP: f32 = 8.0;
/// The checkbox and the radio button.
const BOX: f32 = 18.0;
const BORDER: f32 = 1.5;
/// The switch.
const TRACK_W: f32 = 36.0;
const TRACK_H: f32 = 20.0;
const KNOB: f32 = 16.0;
const KNOB_PAD: f32 = 2.0;
/// How much wider the knob gets while it is pressed.
const KNOB_PRESS: f32 = 4.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    Switch,
    Checkbox,
    Radio,
}

/// What a checkbox shows.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CheckState {
    #[default]
    Off,
    On,
    /// Some of what the box stands for is on and some is off: "select all" over a partial selection.
    /// A click turns everything on.
    Mixed,
}

impl From<bool> for CheckState {
    fn from(on: bool) -> CheckState {
        if on { CheckState::On } else { CheckState::Off }
    }
}

/// The widget behind [`Toggle`], [`Checkbox`] and [`Radio`].
pub struct ControlWidget<M> {
    kind: Kind,
    check: CheckState,
    label: String,
    /// What moving focus onto a radio button with the arrow keys selects it with.
    on_arrow: Option<Rc<dyn Fn() -> M>>,
}

pub struct ControlState {
    /// 0 when off, 1 when on or mixed. It gives a little past either end.
    t: SpringValue,
    baseline: f32,
}

impl<M> ControlWidget<M> {
    fn new(kind: Kind, check: CheckState) -> ControlWidget<M> {
        ControlWidget { kind, check, label: String::new(), on_arrow: None }
    }

    fn goal(&self) -> f32 {
        if self.check == CheckState::Off { 0.0 } else { 1.0 }
    }

    fn control_size(&self) -> Size {
        match self.kind {
            Kind::Switch => size(TRACK_W, TRACK_H),
            Kind::Checkbox | Kind::Radio => size(BOX, BOX),
        }
    }

    fn label_layout(&self, env: &Env) -> Option<Rc<TextLayout>> {
        if self.label.is_empty() {
            return None;
        }
        let mut style = TextStyle::from_token(env.theme.types.body);
        style.wrap = TextWrap::None;
        Some(env.fonts().layout_cached(&self.label, &style, None))
    }
}

/// A box with a fully round end, for knobs, dots and tracks.
fn pill(cx: &mut PaintCx, rect: Rect, color: Color) {
    cx.quad(Quad::new(rect).fill(color.mul_alpha(cx.alpha)).radii(Corners::all(rect.h.min(rect.w) / 2.0)));
}

impl<M: 'static> Widget<M> for ControlWidget<M> {
    type State = ControlState;

    fn create_state(&self) -> ControlState {
        ControlState { t: SpringValue::new(self.goal(), Spring::SNAPPY), baseline: 0.0 }
    }

    fn update(&self, old: &ControlWidget<M>, state: &mut ControlState) -> Dirty {
        if state.t.target() != self.goal() {
            state.t.set_target(self.goal());
        }
        if self.kind != old.kind || self.label != old.label {
            Dirty::Layout
        } else if self.check != old.check {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut ControlState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let control = self.control_size();
        let label = self.label_layout(env);
        let width = control.w + label.as_ref().map_or(0.0, |l| GAP + l.size().w);
        let line = label.as_ref().map_or(0.0, |l| l.size().h);
        let height = control.h.max(line);
        state.baseline = match &label {
            Some(l) => (height - line) / 2.0 + l.baseline(),
            None => control.h,
        };
        size(known.x.unwrap_or(width), known.y.unwrap_or(height))
    }

    fn baseline(&self, state: &ControlState) -> Option<f32> {
        Some(state.baseline)
    }

    fn paint(&self, state: &ControlState, cx: &mut PaintCx) {
        let palette = cx.palette().clone();
        let t = state.t.value();
        let on = t.clamp(0.0, 1.0);
        let c = cx.content;
        let control = self.control_size();
        let at = cx.snap(Rect::new(c.x, c.y + (c.h - control.h) / 2.0, control.w, control.h));

        match self.kind {
            Kind::Switch => {
                pill(cx, at, palette.track_off.lerp(palette.accent, on));
                // The knob widens away from the edge it is near while it is pressed.
                let grow = KNOB_PRESS * cx.press;
                let travel = at.w - 2.0 * KNOB_PAD - KNOB;
                let x = at.x + KNOB_PAD + travel * t - grow * on;
                let knob = cx.snap(Rect::new(x, at.y + (at.h - KNOB) / 2.0, KNOB + grow, KNOB));
                pill(cx, knob, palette.knob);
            }
            Kind::Checkbox | Kind::Radio => {
                let outline = palette.text_3.lerp(palette.text_2, cx.hover).lerp(palette.accent, on);
                let fill = match self.kind {
                    Kind::Checkbox => palette.field.lerp(palette.accent, on),
                    _ => palette.field,
                };
                let radius = if self.kind == Kind::Radio { BOX / 2.0 } else { 5.0 };
                let quad = Quad::new(at).fill(fill.mul_alpha(cx.alpha)).radii(Corners::all(radius)).border(BORDER, outline.mul_alpha(cx.alpha));
                cx.quad(quad);
                if self.kind == Kind::Radio {
                    let d = 8.0 * on;
                    let dot = Rect::new(at.x + (at.w - d) / 2.0, at.y + (at.h - d) / 2.0, d, d);
                    if d > 0.1 {
                        pill(cx, dot, palette.accent);
                    }
                } else if on > 0.01 {
                    let glyph = if self.check == CheckState::Mixed { set::MINUS } else { set::CHECK };
                    let inner = at.deflate(vui_core::Insets::all(2.0));
                    paint_icon(cx, glyph, inner, Fill::Tone(Tone::OnAccent, on), Weight::Bold);
                }
            }
        }

        if let Some(label) = self.label_layout(cx.env) {
            let y = c.y + (c.h - label.size().h) / 2.0;
            let scale = cx.env.scale;
            let snap = |v: f32| (v * scale).round() / scale;
            cx.text(label, pt(snap(at.right() + GAP), snap(y)), Tone::Text);
        }
    }

    fn event(&self, _state: &mut ControlState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        // Arrowing onto a radio button chooses it, as arrowing through a group of them does everywhere.
        if let WidgetEvent::FocusChanged { focused: true, by: FocusBy::Arrow, .. } = event
            && let Some(select) = &self.on_arrow
        {
            cx.emit(select());
        }
        false
    }

    fn tick(&self, state: &mut ControlState, _cx: &mut TickCx, dt: f32) -> bool {
        state.t.step(dt)
    }

    fn animating(&self, state: &ControlState) -> bool {
        !state.t.is_settled()
    }

    fn access(&self, _state: &ControlState, info: &mut AccessInfo) {
        info.role = Some(match self.kind {
            Kind::Switch => Role::Switch,
            Kind::Checkbox => Role::Checkbox,
            Kind::Radio => Role::RadioButton,
        });
        info.label = (!self.label.is_empty()).then(|| self.label.clone());
        info.toggled = Some(self.check == CheckState::On);
        info.mixed = self.check == CheckState::Mixed;
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A switch: 36 by 20, for settings that take effect at once.
pub struct Toggle<M: 'static> {
    base: Base<M>,
    widget: ControlWidget<M>,
}

builder!(Toggle<M> => ControlWidget<M>);

impl<M: 'static> Toggle<M> {
    /// Gets the state a click asks for.
    pub fn on_change(self, f: impl Fn(bool) -> M + 'static) -> Self {
        let next = self.widget.check != CheckState::On;
        self.on_click_with(move || f(next))
    }

    /// A label to the right of the switch. A click on it flips the switch too.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self.size(AUTO, AUTO)
    }
}

/// A switch. On its own it keeps its size inside a stretching parent; with a label it is as wide as
/// its content, or as wide as the parent lets it be.
pub fn toggle<M: 'static>(on: bool) -> Toggle<M> {
    let toggle = Toggle { base: Base::new(), widget: ControlWidget::new(Kind::Switch, on.into()) };
    toggle.rounded(Radius::Pill).cursor(vui_core::CursorIcon::Pointer).size(TRACK_W, TRACK_H)
}

/// A checkbox, with a label if it has one.
pub struct Checkbox<M: 'static> {
    base: Base<M>,
    widget: ControlWidget<M>,
}

builder!(Checkbox<M> => ControlWidget<M>);

impl<M: 'static> Checkbox<M> {
    /// Gets the state a click asks for: a mixed box asks for on.
    pub fn on_change(self, f: impl Fn(bool) -> M + 'static) -> Self {
        let next = self.widget.check != CheckState::On;
        self.on_click_with(move || f(next))
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self.size(AUTO, AUTO)
    }
}

pub fn checkbox<M: 'static>(check: impl Into<CheckState>) -> Checkbox<M> {
    let checkbox = Checkbox { base: Base::new(), widget: ControlWidget::new(Kind::Checkbox, check.into()) };
    checkbox.rounded(Radius::Px(6.0)).cursor(vui_core::CursorIcon::Pointer).size(BOX, BOX)
}

/// A radio button: one of several choices, of which exactly one is on. See [`radio_group`].
pub struct Radio<M: 'static> {
    base: Base<M>,
    widget: ControlWidget<M>,
}

builder!(Radio<M> => ControlWidget<M>);

impl<M: 'static> Radio<M> {
    /// Sent when the button is clicked and when the arrow keys move focus onto it.
    pub fn on_select(mut self, f: impl Fn() -> M + 'static) -> Self {
        let f: Rc<dyn Fn() -> M> = Rc::new(f);
        let again = f.clone();
        self.widget.on_arrow = Some(f);
        self.on_click_with(move || again())
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self.size(AUTO, AUTO)
    }
}

pub fn radio<M: 'static>(selected: bool) -> Radio<M> {
    let radio = Radio { base: Base::new(), widget: ControlWidget::new(Kind::Radio, selected.into()) };
    radio.rounded(Radius::Px(6.0)).cursor(vui_core::CursorIcon::Pointer).size(BOX, BOX)
}

/// A column of radio buttons that is one Tab stop: Tab lands on the chosen one, and the arrow keys move
/// between the choices and choose as they go.
pub fn radio_group<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    let on_select = Rc::new(on_select);
    let mut group = column::<M>().gap(8.0).align_items(Align::Start).focus_group(FocusGroup::vertical()).role(Role::Group);
    for (value, label) in options {
        let chosen = value == *selected;
        let on_select = on_select.clone();
        group = group.child(radio(chosen).label(label).tab_default(chosen).on_select(move || on_select(value.clone())));
    }
    group
}
