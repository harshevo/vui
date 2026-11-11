//! The widget trait and what widgets are given to work with.
//!
//! A widget is the content of an element: text, an image, a toggle's knob. The box around it
//! (background, border, hover and press fills, focus ring, click handlers, layout) belongs to the
//! element and is drawn and routed by the runtime, so a widget only deals with what is inside.
//!
//! Widgets are values rebuilt by the app's `view` on every update. State that must survive
//! (a text field's caret, a toggle's animated knob) lives in the widget's `State`, which the runtime
//! keeps for as long as the node stays in the tree.

use std::any::{Any, TypeId};
use std::rc::Rc;

use vui_core::{Color, Command, Corners, CustomCmd, CursorIcon, Event, Fill, Fonts, Images, ImageCmd, Modifiers, Palette, Point, Quad, Rect, Resources, Scene, Size, TextLayout, Theme};

use crate::access::{AccessAction, AccessInfo};
use crate::effects::{Clipboard, Effect};
use crate::key::NodeId;
use crate::layout::{Avail, Pair};

mod context;
mod erased;

pub use context::{EventCx, PaintCx, TickCx};
pub(crate) use context::Output;
pub(crate) use erased::{Erased, Mapped, Plain};

/// How much of a node must be redone after its props changed.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dirty {
    #[default]
    None,
    /// Only what is drawn.
    Paint,
    /// Sizes and positions too.
    Layout,
}

/// Whether and how an element takes keyboard focus.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    /// Skipped by Tab and never focused.
    #[default]
    Never,
    /// Reached with Tab. Clicking it leaves focus where it is, so a toolbar button does not take the
    /// caret out of a text field.
    Keyboard,
    /// Reached with Tab, and focused by a click too: text fields.
    Click,
}

/// What moved focus. A radio group selects the option an arrow key lands on but not the one Tab lands on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FocusBy {
    /// A click or tap.
    Pointer,
    /// The Tab key.
    Tab,
    /// An arrow key, Home or End inside a focus group.
    Arrow,
    /// Anything else: the app asked, or the widget did.
    #[default]
    Other,
}

/// The shared services widgets measure and paint with.
#[derive(Clone)]
pub struct Env {
    pub res: Resources,
    pub theme: Rc<Theme>,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// The window's width in logical pixels, which responsive rules are matched against.
    pub window: f32,
    /// The user's accessibility settings. `theme` already follows them; widgets that move on their
    /// own hold still when `reduce_motion` is on.
    pub prefs: vui_core::Preferences,
    /// The theme as designed, before `prefs` adapted it.
    pub(crate) designed: Rc<Theme>,
}

impl Env {
    pub fn new(res: Resources, theme: Theme, scale: f32) -> Env {
        let designed = Rc::new(theme);
        Env { res, theme: designed.clone(), scale, window: 0.0, prefs: vui_core::Preferences::DEFAULT, designed }
    }

    pub fn fonts(&self) -> &Fonts {
        &self.res.fonts
    }

    pub fn images(&self) -> &Images {
        &self.res.images
    }

    pub fn palette(&self) -> &Palette {
        &self.theme.palette
    }
}

/// What happened to a widget. More kinds will be added, so match with a catch-all arm.
#[derive(Debug)]
#[non_exhaustive]
pub enum WidgetEvent<'a> {
    /// Input aimed at the widget: pointer events over it (all of them while it holds the capture),
    /// scrolling over it, and keys, text and input-method events while it has focus. Positions are in
    /// window coordinates; `EventCx::to_local` converts.
    Input(&'a Event),
    /// The pointer entered or left it.
    HoverChanged(bool),
    /// Focus arrived or left. `visible` is true when it came by keyboard, which is when a focus ring
    /// is wanted. `by` is what moved it, on the element losing focus as well as the one gaining it.
    FocusChanged { focused: bool, visible: bool, by: FocusBy },
    /// The window became or stopped being the key window. Only the focused widget hears it: a text
    /// field hides its caret.
    WindowFocus(bool),
    /// Assistive technology asked it to do something.
    Action(&'a AccessAction),
}

/// The content of an element.
pub trait Widget<M>: Sized + 'static {
    /// What the runtime keeps for the node between updates.
    type State: 'static;

    fn create_state(&self) -> Self::State;

    /// Called when the app's `view` produced this widget for a node that already existed, with the
    /// widget it replaces. Say how much changed; the default assumes sizes did.
    fn update(&self, old: &Self, state: &mut Self::State) -> Dirty {
        let _ = (old, state);
        Dirty::Layout
    }

    /// The content size of a node with no children. `known` is the content-box size already decided on
    /// each axis and `avail` the room offered; padding is added by the runtime.
    fn measure(&self, state: &mut Self::State, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let _ = (state, env, known, avail);
        Size::ZERO
    }

    /// The first baseline, from the top of the content box, as of the latest `measure`.
    fn baseline(&self, state: &Self::State) -> Option<f32> {
        let _ = state;
        None
    }

    /// Draws the content, after the element's background and border and before its children.
    fn paint(&self, state: &Self::State, cx: &mut PaintCx) {
        let _ = (state, cx);
    }

    /// Draws over the children.
    fn paint_over(&self, state: &Self::State, cx: &mut PaintCx) {
        let _ = (state, cx);
    }

    /// Handles an event. Returns true if it was used, which stops it bubbling to the node's parents.
    fn event(&self, state: &mut Self::State, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let _ = (state, cx, event);
        false
    }

    /// Advances animations by `dt` seconds. Returns true while something is still moving.
    fn tick(&self, state: &mut Self::State, cx: &mut TickCx, dt: f32) -> bool {
        let _ = (state, cx, dt);
        false
    }

    /// Whether an animation is waiting to run. Asked after each `update`, so a widget that points a
    /// spring at a new target there gets the frames to play it; `tick` takes over from then on.
    fn animating(&self, state: &Self::State) -> bool {
        let _ = state;
        false
    }

    /// Describes the widget to assistive technology.
    fn access(&self, state: &Self::State, info: &mut AccessInfo) {
        let _ = (state, info);
    }

    /// Whether the widget takes keyboard focus by itself, like a text field. The element can override it.
    fn focusable(&self) -> Focus {
        Focus::Never
    }

    /// The pointer shape while it is over the widget.
    fn cursor(&self) -> Option<CursorIcon> {
        None
    }
}
