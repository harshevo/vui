//! A splitter: the handle between two panes that drags the edge between them. It stands between
//! panes in a row; `.vertical()` lays it across, between panes in a column.
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
    /// Between panes in a column: it drags up and down.
    across: bool,
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
        let (more, less) = if self.across { (NamedKey::ArrowDown, NamedKey::ArrowUp) } else { (NamedKey::ArrowRight, NamedKey::ArrowLeft) };
        match key.key {
            Key::Named(k) if k == more => self.send(cx, KEY_STEP),
            Key::Named(k) if k == less => self.send(cx, -KEY_STEP),
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

    fn update(&self, old: &SplitterWidget<M>, _state: &mut SplitterState) -> Dirty {
        if old.across != self.across { Dirty::Layout } else { Dirty::None }
    }

    fn measure(&self, _state: &mut SplitterState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let along = |a: Avail| match a {
            Avail::Definite(v) => v,
            _ => 0.0,
        };
        if self.across {
            size(known.x.unwrap_or(along(avail.x)), known.y.unwrap_or(WIDTH))
        } else {
            size(known.x.unwrap_or(WIDTH), known.y.unwrap_or(along(avail.y)))
        }
    }

    fn paint(&self, state: &SplitterState, cx: &mut PaintCx) {
        let strength = if state.grab.is_some() { 1.0 } else { cx.hover.max(cx.focus) };
        if strength <= 0.01 {
            return;
        }
        let c = cx.content;
        let line = if self.across { Rect::new(c.x, c.y + (c.h - LINE) / 2.0, c.w, LINE) } else { Rect::new(c.x + (c.w - LINE) / 2.0, c.y, LINE, c.h) };
        let line = cx.snap(line);
        let colour = cx.palette().line.mul_alpha(strength * cx.alpha);
        cx.quad(Quad::new(line).fill(colour));
    }

    fn event(&self, state: &mut SplitterState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.grab = Some(if self.across { pos.y - cx.content.y } else { pos.x - cx.content.x });
                cx.capture_pointer();
                cx.request_paint();
                true
            }
            // The handle moves with the edge, so measuring from where it is now keeps it under the
            // pointer even when the app stops the edge at a limit.
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.grab.is_some() => {
                let grab = state.grab.unwrap_or(0.0);
                let at = if self.across { pos.y - cx.content.y } else { pos.x - cx.content.x };
                self.send(cx, at - grab);
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
    /// Called with how far the edge should move, in points: positive is to the right (down, when
    /// vertical).
    pub fn on_drag(mut self, f: impl Fn(f32) -> M + 'static) -> Self {
        self.widget.on_drag = Some(Rc::new(f));
        self
    }

    /// Lies across, between panes in a column, and drags up and down.
    pub fn vertical(mut self) -> Self {
        self.widget.across = true;
        self.cursor(CursorIcon::ResizeVertical).shrink(0.0)
    }
}

/// A vertical splitter. Name it for assistive technology: `splitter().name("Resize projects")`.
pub fn splitter<M: 'static>() -> Splitter<M> {
    let splitter = Splitter { base: Base::new(), widget: SplitterWidget { on_drag: None, across: false } };
    splitter.focus_ring(false).cursor(CursorIcon::ResizeHorizontal).shrink(0.0)
}
