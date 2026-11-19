//! Hold to confirm: a button that acts only after it has been held down for a moment. A fill sweeps
//! across while it is held; letting go early drains it and nothing happens. It replaces an "Are you
//! sure?" dialog for actions that are hard to undo.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, Key, NamedKey, PointerButton, Rect, Size, TextRole, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Radius, Role, TickCx, Widget, WidgetEvent, builder};

use super::line;

const PAD: f32 = 10.0;
/// How fast an unfinished hold drains, in holds per second.
const DRAIN: f32 = 4.0;
/// How long the full fill stays after it fires.
const DONE: f32 = 0.5;

/// The widget behind [`Hold`].
pub struct HoldWidget<M> {
    label: String,
    tone: Tone,
    seconds: f32,
    height: f32,
    /// No wash at rest, only the coloured word: for a toolbar where it must not compete.
    quiet: bool,
    on_hold: Option<Rc<dyn Fn() -> M>>,
}

#[derive(Default)]
pub struct HoldState {
    /// 0 to 1: how far the fill has swept.
    progress: f32,
    holding: bool,
    fired: bool,
    /// Seconds left showing the full fill after it fired.
    done: f32,
    /// How long it has been held so far this time.
    held: f32,
    /// Seconds left of the head shake that says a click is not a hold.
    hint: f32,
}

impl<M> HoldWidget<M> {
    fn start(&self, state: &mut HoldState, cx: &mut EventCx<M>) {
        if !state.holding && state.done <= 0.0 {
            state.holding = true;
            state.fired = false;
            state.held = 0.0;
            cx.request_frame();
        }
    }
}

impl<M: 'static> Widget<M> for HoldWidget<M> {
    type State = HoldState;

    fn create_state(&self) -> HoldState {
        HoldState::default()
    }

    fn update(&self, old: &HoldWidget<M>, _state: &mut HoldState) -> Dirty {
        if self.label != old.label || self.height != old.height {
            Dirty::Layout
        } else if self.tone != old.tone {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, _state: &mut HoldState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let w = line(env, &self.label, TextRole::Small, 500, false).size().w;
        size(known.x.unwrap_or(w + PAD * 2.0), known.y.unwrap_or(self.height))
    }

    fn paint(&self, state: &HoldState, cx: &mut PaintCx) {
        let shift = super::shake_offset(state.hint, cx.env.prefs.reduce_motion);
        let mut rect = cx.rect;
        rect.x += shift;
        let radii = cx.radii;
        // The resting button: the tone's own text on a faint wash of it.
        let rest = if self.quiet { 0.0 } else { 0.10 };
        cx.fill_rounded(rect, radii, Fill::Tone(self.tone, rest + 0.08 * cx.hover));
        // While it shakes it says what it wants.
        let text = if state.hint > 0.0 { "Hold" } else { self.label.as_str() };
        let layout = line(cx.env, text, TextRole::Small, 500, false);
        let s = layout.size();
        let at = pt(rect.x + (rect.w - s.w) / 2.0, rect.y + (rect.h - s.h) / 2.0);
        cx.text(layout.clone(), at, self.tone);
        // The sweep, left to right, with the label in white where it has passed.
        let p = state.progress.clamp(0.0, 1.0);
        if p > 0.0 {
            let swept = Rect::new(rect.x, rect.y, rect.w * p, rect.h);
            let on = if self.tone == Tone::Accent { Tone::OnAccent } else { Tone::OnDanger };
            cx.clipped(swept, |cx| {
                cx.fill_rounded(rect, radii, self.tone);
                cx.text(layout, at, on);
            });
        }
    }

    fn event(&self, state: &mut HoldState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { button: PointerButton::Primary, .. }) => {
                cx.capture_pointer();
                self.start(state, cx);
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => {
                cx.release_pointer();
                if state.holding && !state.fired && state.held < self.seconds * 0.5 {
                    // A click, not a hold: shake and say so, instead of doing nothing.
                    state.hint = super::SHAKE;
                }
                state.holding = false;
                cx.request_frame();
                true
            }
            WidgetEvent::Input(Event::Key(key)) if matches!(key.key, Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter)) => {
                if key.pressed {
                    if !key.repeat {
                        self.start(state, cx);
                    }
                } else {
                    state.holding = false;
                    cx.request_frame();
                }
                true
            }
            WidgetEvent::FocusChanged { focused: false, .. } => {
                state.holding = false;
                cx.request_frame();
                false
            }
            // Assistive technology cannot hold, so its press counts at once.
            WidgetEvent::Action(AccessAction::Click) => {
                if let Some(f) = &self.on_hold {
                    cx.emit(f());
                }
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut HoldState, cx: &mut TickCx, dt: f32) -> bool {
        state.hint = (state.hint - dt).max(0.0);
        if state.holding && !state.fired {
            state.held += dt;
            state.progress += dt / self.seconds.max(0.05);
            if state.progress >= 1.0 {
                state.progress = 1.0;
                state.fired = true;
                state.done = DONE;
                if let Some(f) = &self.on_hold {
                    cx.emit(f());
                }
            }
        } else if state.done > 0.0 {
            state.done -= dt;
        } else if !state.holding && state.progress > 0.0 {
            state.progress = (state.progress - dt * DRAIN).max(0.0);
        }
        cx.request_paint();
        state.holding || state.done > 0.0 || state.progress > 0.0 || state.hint > 0.0
    }

    fn animating(&self, state: &HoldState) -> bool {
        state.holding || state.done > 0.0 || state.progress > 0.0 || state.hint > 0.0
    }

    fn access(&self, _state: &HoldState, info: &mut AccessInfo) {
        info.role = Some(Role::Button);
        info.label = Some(self.label.clone());
        info.actions = vec![AccessActionKind::Click];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}

/// A button that acts once it has been held.
pub struct Hold<M: 'static> {
    base: Base<M>,
    widget: HoldWidget<M>,
}

builder!(Hold<M> => HoldWidget<M>);

impl<M: 'static> Hold<M> {
    /// How long to hold. 0.6 s unless told.
    pub fn seconds(mut self, seconds: f32) -> Self {
        self.widget.seconds = seconds;
        self
    }

    /// The accent instead of red, for something weighty that is not destructive.
    pub fn accent(mut self) -> Self {
        self.widget.tone = Tone::Accent;
        self
    }

    /// 24 high instead of 28.
    pub fn small(mut self) -> Self {
        self.widget.height = 24.0;
        self
    }

    /// Only the coloured word until the pointer is over it.
    pub fn quiet(mut self) -> Self {
        self.widget.quiet = true;
        self
    }
}

/// A red button that sends `on_hold` once it has been held for 0.6 s: `hold_button("Kill switch",
/// Msg::Kill)`. Say in its tooltip that it wants holding.
pub fn hold_button<M: Clone + 'static>(label: impl Into<String>, on_hold: M) -> Hold<M> {
    let label = label.into();
    let widget = HoldWidget { label: label.clone(), tone: Tone::Danger, seconds: 0.6, height: 28.0, quiet: false, on_hold: Some(Rc::new(move || on_hold.clone())) };
    Hold { base: Base::new(), widget }.rounded(Radius::Pill).name(label)
}
