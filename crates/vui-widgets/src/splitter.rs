//! A splitter: the handle between two panes that drags the edge between them.
//!
//! Like every widget here it is controlled: the app owns the pane's width and changes it by what the
//! splitter sends. A thin line shows on hover and while dragging.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Key, KeyEvent, NamedKey, PointerButton, Quad, Rect, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, Styled, Widget,
    WidgetEvent,
    builder,
};

/// How wide the handle is: easy to grab, and the gap between the panes.
const WIDTH: f32 = 12.0;
/// The line drawn down the middle of the handle.
const LINE: f32 = 2.0;
/// What one arrow key press moves the edge by.
const KEY_STEP: f32 = 16.0;

type Handler<M> = Rc<dyn Fn(f32) -> M>;

/// The widget behind [`Splitter`].
pub struct SplitterWidget<M> {
    on_drag: Option<Handler<M>>,
}

#[derive(Default)]
pub struct SplitterState {
    /// While dragging: where in the handle it was grabbed, so the edge does not jump to the pointer.
    grab: Option<f32>,
}

impl<M> SplitterWidget<M> {
    fn send(&self, cx: &mut EventCx<M>, by: f32) {
        if by != 0.0
            && let Some(on_drag) = &self.on_drag
        {
            cx.emit(on_drag(by));
        }
    }

    fn key(&self, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed || key.mods.cmd || key.mods.ctrl || key.mods.alt {
            return false;
        }
        match key.key {
            Key::Named(NamedKey::ArrowRight) => self.send(cx, KEY_STEP),
            Key::Named(NamedKey::ArrowLeft) => self.send(cx, -KEY_STEP),
            _ => return false,
        }
        true
    }
}

impl<M: 'static> Widget<M> for SplitterWidget<M> {
    type State = SplitterState;

    fn create_state(&self) -> SplitterState {
        SplitterState::default()
    }

    fn update(&self, _old: &SplitterWidget<M>, _state: &mut SplitterState) -> Dirty {
        Dirty::None
    }

    fn measure(&self, _state: &mut SplitterState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let height = match avail.y {
            Avail::Definite(h) => h,
            _ => 0.0,
        };
        size(known.x.unwrap_or(WIDTH), known.y.unwrap_or(height))
    }

    fn paint(&self, state: &SplitterState, cx: &mut PaintCx) {
        let strength = if state.grab.is_some() { 1.0 } else { cx.hover.max(cx.focus) };
        if strength <= 0.01 {
            return;
        }
        let c = cx.content;
        let line = cx.snap(Rect::new(c.x + (c.w - LINE) / 2.0, c.y, LINE, c.h));
        let colour = cx.palette().line.mul_alpha(strength * cx.alpha);
        cx.quad(Quad::new(line).fill(colour));
    }

    fn event(&self, state: &mut SplitterState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.grab = Some(pos.x - cx.content.x);
                cx.capture_pointer();
                cx.request_paint();
                true
            }
            // The handle moves with the edge, so measuring from where it is now keeps it under the
            // pointer even when the app stops the edge at a limit.
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.grab.is_some() => {
                let grab = state.grab.unwrap_or(0.0);
                self.send(cx, pos.x - cx.content.x - grab);
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) if state.grab.is_some() => {
                state.grab = None;
                cx.release_pointer();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Key(key)) => self.key(cx, key),
            WidgetEvent::Action(AccessAction::Increment) => {
                self.send(cx, KEY_STEP);
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                self.send(cx, -KEY_STEP);
                true
            }
            _ => false,
        }
    }

    fn access(&self, _state: &SplitterState, info: &mut AccessInfo) {
        info.role = Some(Role::Separator);
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }
}

/// A handle that resizes the pane before it.
pub struct Splitter<M: 'static> {
    base: Base<M>,
    widget: SplitterWidget<M>,
}

builder!(Splitter<M> => SplitterWidget<M>);

impl<M: 'static> Splitter<M> {
    /// Called with how far the edge should move, in points: positive is to the right.
    pub fn on_drag(mut self, f: impl Fn(f32) -> M + 'static) -> Self {
        self.widget.on_drag = Some(Rc::new(f));
        self
    }
}

/// A vertical splitter. Name it for assistive technology: `splitter().name("Resize projects")`.
pub fn splitter<M: 'static>() -> Splitter<M> {
    let splitter = Splitter { base: Base::new(), widget: SplitterWidget { on_drag: None } };
    splitter.focus_ring(false).cursor(CursorIcon::ResizeHorizontal).shrink(0.0)
}
