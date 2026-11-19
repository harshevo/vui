//! The slot: a one-word drum. It shows the chosen option alone; the wheel, a vertical drag, the
//! arrow keys or a click roll the next one into place, and a spring settles it.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, Key, NamedKey, PointerButton, Rect, Size, Spring, SpringValue, TextLayout, TextRole, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Radius, Role, TickCx, Widget, WidgetEvent,
    builder,
};

use super::line;

/// Room either side of the word; the right side also holds the small up-down chevron that says it
/// rolls.
const PAD: f32 = 8.0;
const DOTS: f32 = 12.0;
/// How far the wheel or a drag goes for one step.
const STEP: f32 = 22.0;
/// How far apart the words sit on the drum.
const ROW: f32 = 18.0;
/// How hard the drum bounces when it is pushed past either end.
const KICK: f32 = 7.0;
/// How hard it turns the first time the pointer comes over it.
const NUDGE: f32 = 6.0;

type Pick<M> = Rc<dyn Fn(usize) -> M>;

/// The widget behind [`Slot`].
pub struct SlotWidget<M> {
    labels: Vec<String>,
    chosen: usize,
    mono: bool,
    height: f32,
    on_pick: Option<Pick<M>>,
}

pub struct SlotState {
    layouts: Vec<Rc<TextLayout>>,
    /// Where the drum is, as an option index; a spring pulls it to the chosen one.
    roll: SpringValue,
    /// Wheel travel not yet turned into a step.
    wheel: f32,
    /// While the pointer is down: where it last stepped from, and whether it has stepped at all.
    drag: Option<(f32, bool)>,
    /// Whether the drum has shown that it turns, which it does once, the first time the pointer
    /// comes over it.
    shown: bool,
}

impl<M> SlotWidget<M> {
    fn pick(&self, cx: &mut EventCx<M>, state: &mut SlotState, by: i32, wrap: bool) {
        let n = self.labels.len() as i32;
        if n == 0 {
            return;
        }
        let mut to = self.chosen as i32 + by;
        if wrap {
            to = to.rem_euclid(n);
        }
        if !(0..n).contains(&to) {
            // Past the end: the drum gives a little and springs back.
            state.roll.set_velocity(KICK * by.signum() as f32);
            cx.request_frame();
            return;
        }
        if to as usize != self.chosen
            && let Some(on_pick) = &self.on_pick
        {
            cx.emit(on_pick(to as usize));
        }
    }
}

impl<M: 'static> Widget<M> for SlotWidget<M> {
    type State = SlotState;

    fn create_state(&self) -> SlotState {
        SlotState { layouts: Vec::new(), roll: SpringValue::new(self.chosen as f32, Spring::SNAPPY), wheel: 0.0, drag: None, shown: false }
    }

    fn update(&self, old: &SlotWidget<M>, state: &mut SlotState) -> Dirty {
        if self.chosen != old.chosen {
            state.roll.set_target(self.chosen as f32);
        }
        if self.labels != old.labels || self.mono != old.mono || self.height != old.height {
            Dirty::Layout
        } else if self.chosen != old.chosen {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut SlotState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        state.layouts = self.labels.iter().map(|l| line(env, l, TextRole::Small, 500, self.mono)).collect();
        // As wide as the widest word, so the slot does not jump as it rolls.
        let widest = state.layouts.iter().map(|l| l.size().w).fold(0.0, f32::max);
        size(known.x.unwrap_or(widest + PAD * 2.0 + DOTS), known.y.unwrap_or(self.height))
    }

    fn paint(&self, state: &SlotState, cx: &mut PaintCx) {
        let content = cx.content;
        let rect = cx.rect;
        let roll = state.roll.value();
        let mid = content.y + content.h / 2.0;
        let room = content.w - PAD - DOTS;
        let (hover, layouts) = (cx.hover, &state.layouts);
        cx.clipped(rect, |cx| {
            for (k, layout) in layouts.iter().enumerate() {
                let d = k as f32 - roll;
                if d.abs() > 1.6 {
                    continue;
                }
                // The chosen word at full strength; its neighbours peek in faintly while hovered,
                // and fade out towards the edges as the drum turns.
                let near = (1.0 - d.abs()).max(0.0);
                let peek = if d.abs() <= 1.2 { hover * 0.3 * (1.0 - near) } else { 0.0 };
                let alpha = (near * near).max(peek);
                if alpha <= 0.01 {
                    continue;
                }
                let text = layout.size();
                let at = pt(content.x + PAD + (room - text.w) / 2.0, mid + d * ROW - text.h / 2.0);
                let tone = if near > 0.5 { Tone::Text } else { Tone::Text3 };
                cx.text(layout.clone(), at, Fill::Tone(tone, alpha));
            }
        });
        // A small up-down chevron on the right: there is more above and below.
        let at = Rect::new(content.x + content.w - DOTS, mid - 6.0, 12.0, 12.0);
        crate::display::icon::paint_icon(cx, vui_icons::set::CHEVRONS_UP_DOWN, at, Fill::Tone(Tone::Text3, 0.7 + 0.3 * cx.hover), vui_icons::Weight::Regular);
    }

    fn event(&self, state: &mut SlotState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::HoverChanged(true) if !state.shown => {
                // A small turn towards the next word and back: this is a drum, not a label.
                state.shown = true;
                let towards = if self.chosen + 1 < self.labels.len() { 1.0 } else { -1.0 };
                state.roll.set_velocity(NUDGE * towards);
                cx.request_frame();
                false
            }
            WidgetEvent::Input(Event::Scroll { delta, .. }) => {
                // Content moving up (the wheel turned towards you) brings the next option in.
                state.wheel += delta.y;
                while state.wheel <= -STEP {
                    state.wheel += STEP;
                    self.pick(cx, state, 1, false);
                }
                while state.wheel >= STEP {
                    state.wheel -= STEP;
                    self.pick(cx, state, -1, false);
                }
                true
            }
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.drag = Some((pos.y, false));
                cx.capture_pointer();
                cx.focus();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                let Some((from, _)) = state.drag else { return false };
                let dy = pos.y - from;
                if dy.abs() >= STEP {
                    // Dragging up pulls the drum up, which brings the next option.
                    self.pick(cx, state, if dy < 0.0 { 1 } else { -1 }, false);
                    state.drag = Some((pos.y, true));
                }
                true
            }
            WidgetEvent::Input(Event::PointerUp { mods, .. }) => {
                let Some((_, moved)) = state.drag.take() else { return false };
                cx.release_pointer();
                if !moved {
                    // A click turns to the next option, and Shift-click to the one before.
                    self.pick(cx, state, if mods.shift { -1 } else { 1 }, true);
                }
                true
            }
            WidgetEvent::Input(Event::Key(key)) if key.pressed => match key.key {
                Key::Named(NamedKey::ArrowUp) => {
                    self.pick(cx, state, -1, false);
                    true
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.pick(cx, state, 1, false);
                    true
                }
                Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) => {
                    self.pick(cx, state, 1, true);
                    true
                }
                _ => false,
            },
            WidgetEvent::Action(AccessAction::Increment) | WidgetEvent::Action(AccessAction::Click) => {
                self.pick(cx, state, 1, true);
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.pick(cx, state, -1, true);
                true
            }
            WidgetEvent::Action(AccessAction::SetValue(label)) => {
                if let Some(i) = self.labels.iter().position(|l| l == label) {
                    self.pick(cx, state, i as i32 - self.chosen as i32, false);
                }
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut SlotState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.roll, dt)
    }

    fn animating(&self, state: &SlotState) -> bool {
        !state.roll.is_settled()
    }

    fn access(&self, _state: &SlotState, info: &mut AccessInfo) {
        info.role = Some(Role::Button);
        info.value = self.labels.get(self.chosen).cloned();
        info.actions = vec![AccessActionKind::Click, AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::ResizeVertical)
    }
}

/// One word for a mode out of a few, which rolls to the next on wheel, drag, arrow keys or click.
pub struct Slot<M: 'static> {
    base: Base<M>,
    widget: SlotWidget<M>,
}

builder!(Slot<M> => SlotWidget<M>);

impl<M: 'static> Slot<M> {
    /// The words in the monospaced face, for codes such as MIS / CNC.
    pub fn mono(mut self) -> Self {
        self.widget.mono = true;
        self
    }

    /// 24 high instead of 28.
    pub fn small(mut self) -> Self {
        self.widget.height = 24.0;
        self
    }
}

/// `(value, label)` for each option, the chosen value, and what to send when the drum turns. Name it
/// with a tooltip, since it shows only the word: `slot(modes, &mode, Msg::Mode).tooltip("Mode")`.
pub fn slot<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, impl Into<String>)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Slot<M> {
    let (values, labels): (Vec<T>, Vec<String>) = options.into_iter().map(|(v, l)| (v, l.into())).unzip();
    let chosen = values.iter().position(|v| v == selected).unwrap_or(0);
    let on_pick: Pick<M> = Rc::new(move |i| on_select(values[i].clone()));
    let widget = SlotWidget { labels, chosen, mono: false, height: 28.0, on_pick: Some(on_pick) };
    Slot { base: Base::new(), widget }.rounded(Radius::Control).hover_bg(Tone::Hover)
}
