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
}

impl Env {
    pub fn new(res: Resources, theme: Theme, scale: f32) -> Env {
        Env { res, theme: Rc::new(theme), scale, window: 0.0 }
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

/// What a widget asked for during a callback. The runtime acts on it once the callback returns.
#[derive(Default)]
pub(crate) struct Requests {
    /// `Some(true)` takes focus, `Some(false)` gives it up.
    pub focus: Option<bool>,
    pub capture: Option<bool>,
    pub layout: bool,
    pub paint: bool,
    pub frame: bool,
    pub wake: Option<f32>,
    pub reveal: Option<Rect>,
}

pub(crate) struct Output<M> {
    pub messages: Vec<M>,
    pub effects: Vec<Effect>,
    pub req: Requests,
}

impl<M> Default for Output<M> {
    fn default() -> Output<M> {
        Output { messages: Vec::new(), effects: Vec::new(), req: Requests::default() }
    }
}

/// What a widget can see and do while handling an event.
pub struct EventCx<'a, M> {
    pub env: &'a Env,
    pub id: NodeId,
    /// The node's border box and content box, in window coordinates.
    pub rect: Rect,
    pub content: Rect,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub focus_visible: bool,
    pub window_focused: bool,
    pub mods: Modifiers,
    /// Seconds since the UI started, advanced by `Ui::tick`.
    pub time: f32,
    pub(crate) out: &'a mut Output<M>,
    pub(crate) clipboard: &'a dyn Clipboard,
}

impl<M> EventCx<'_, M> {
    /// Hands a message to the app's `update`.
    pub fn emit(&mut self, message: M) {
        self.out.messages.push(message);
    }

    /// Takes keyboard focus.
    pub fn focus(&mut self) {
        self.out.req.focus = Some(true);
    }

    pub fn blur(&mut self) {
        self.out.req.focus = Some(false);
    }

    /// Receives every pointer event until the button is released, wherever the pointer goes: how
    /// sliders and text selection follow a drag out of their bounds.
    pub fn capture_pointer(&mut self) {
        self.out.req.capture = Some(true);
    }

    pub fn release_pointer(&mut self) {
        self.out.req.capture = Some(false);
    }

    /// Sizes changed: lay the tree out again.
    pub fn request_layout(&mut self) {
        self.out.req.layout = true;
    }

    /// Only what is drawn changed.
    pub fn request_paint(&mut self) {
        self.out.req.paint = true;
    }

    /// Keep calling `tick` every frame: an animation is running.
    pub fn request_frame(&mut self) {
        self.out.req.frame = true;
    }

    /// Call `tick` again in `seconds`, without running frames in between: a blinking caret.
    pub fn request_wake(&mut self, seconds: f32) {
        let wake = self.out.req.wake.map_or(seconds, |w| w.min(seconds));
        self.out.req.wake = Some(wake);
    }

    /// Where the text caret is, in window coordinates, so the input method can place its window.
    /// `None` when nothing is being edited.
    pub fn set_ime_area(&mut self, area: Option<Rect>) {
        self.out.effects.push(Effect::ImeArea(area));
    }

    /// Scrolls enclosing scroll containers until `rect` (window coordinates) is in view.
    pub fn scroll_into_view(&mut self, rect: Rect) {
        self.out.req.reveal = Some(rect);
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard.get()
    }

    pub fn set_clipboard_text(&self, text: &str) {
        self.clipboard.set(text);
    }

    /// A picture on the clipboard, as PNG bytes.
    pub fn clipboard_image(&self) -> Option<Vec<u8>> {
        self.clipboard.image()
    }

    /// A window position relative to the node's border box.
    pub fn to_local(&self, p: Point) -> Point {
        Point { x: p.x - self.rect.x, y: p.y - self.rect.y }
    }
}

/// What a widget can do while time passes.
pub struct TickCx<'a> {
    pub env: &'a Env,
    pub time: f32,
    pub window_focused: bool,
    pub focused: bool,
    pub(crate) layout: bool,
    pub(crate) paint: bool,
    pub(crate) wake: Option<f32>,
}

impl TickCx<'_> {
    pub fn request_layout(&mut self) {
        self.layout = true;
    }

    pub fn request_paint(&mut self) {
        self.paint = true;
    }

    /// Call `tick` again in `seconds`.
    pub fn request_wake(&mut self, seconds: f32) {
        self.wake = Some(self.wake.map_or(seconds, |w| w.min(seconds)));
    }
}

/// What a widget paints with: the scene to add to, where the node is, and how it is being interacted
/// with. Colours given as a [`Fill`] follow the theme and the element's opacity.
pub struct PaintCx<'a> {
    pub scene: &'a mut Scene,
    pub env: &'a Env,
    /// The node's border box and content box, in window coordinates, moved onto the device pixel grid
    /// so that quads and images have crisp edges.
    pub rect: Rect,
    pub content: Rect,
    /// The content box as layout left it, before moving onto the pixel grid. Measure and wrap text to
    /// this one: moving the edges can make a box that was sized to its text a fraction of a pixel too
    /// narrow to hold it.
    pub exact: Rect,
    /// Everything outside this rect is cut away.
    pub clip: Option<Rect>,
    /// The element's own corner radii, resolved against the theme, and how continuous they are.
    pub radii: Corners,
    pub smooth: f32,
    /// Opacity of the element and everything above it.
    pub alpha: f32,
    /// 0 to 1, animated: the pointer is over it, it is pressed, it shows a focus ring.
    pub hover: f32,
    pub press: f32,
    pub focus: f32,
    pub focused: bool,
    pub window_focused: bool,
    pub disabled: bool,
    /// Seconds since the UI started.
    pub time: f32,
}

impl PaintCx<'_> {
    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }

    pub fn palette(&self) -> &Palette {
        &self.env.theme.palette
    }

    pub fn fonts(&self) -> &Fonts {
        &self.env.res.fonts
    }

    /// A fill resolved against the theme and faded by the element's opacity.
    pub fn color(&self, fill: impl Into<Fill>) -> Color {
        fill.into().resolve(self.palette()).mul_alpha(self.alpha)
    }

    /// Rounds a rect's edges to whole device pixels, so lines stay crisp.
    pub fn snap(&self, rect: Rect) -> Rect {
        rect.snap(self.env.scale)
    }

    /// Runs `f` with everything outside `rect` cut away as well: text that scrolls inside a field.
    pub fn clipped(&mut self, rect: Rect, f: impl FnOnce(&mut PaintCx)) {
        let saved = self.clip;
        self.clip = Some(saved.map_or(rect, |c| c.intersection(rect).unwrap_or(Rect::ZERO)));
        f(self);
        self.clip = saved;
    }

    /// Adds a quad, cut to the clip.
    pub fn quad(&mut self, quad: Quad) {
        self.scene.quad(quad.clip(self.clip));
    }

    /// Fills `rect` with the element's corner radii.
    pub fn fill_rect(&mut self, rect: Rect, fill: impl Into<Fill>) {
        self.fill_rounded(rect, self.radii, fill);
    }

    pub fn fill_rounded(&mut self, rect: Rect, radii: Corners, fill: impl Into<Fill>) {
        let color = self.color(fill);
        if color.a > 0.0 {
            self.quad(Quad::new(rect).fill(color).radii(radii).smooth(self.smooth));
        }
    }

    /// Adds an image, cut to the clip and faded by the element's opacity.
    pub fn image(&mut self, mut cmd: ImageCmd) {
        cmd.clip = self.clip;
        cmd.opacity *= self.alpha;
        self.scene.image(cmd);
    }

    /// Adds drawing for a custom shader pass, cut to the clip and faded by the element's opacity.
    pub fn custom(&mut self, mut cmd: CustomCmd) {
        cmd.clip = self.clip;
        cmd.opacity *= self.alpha;
        self.scene.push(Command::Custom(cmd));
    }

    /// Adds shaped text with its top-left corner at `origin`.
    pub fn text(&mut self, layout: Rc<TextLayout>, origin: Point, fill: impl Into<Fill>) {
        let color = self.color(fill);
        if color.a > 0.0 {
            self.scene.text(layout, origin, color, self.clip);
        }
    }
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

/// A widget with its state type erased, so nodes can hold any kind.
pub(crate) trait Erased<M: 'static>: 'static {
    fn as_any(&self) -> &dyn Any;
    fn widget_type(&self) -> TypeId;
    fn create_state(&self) -> Box<dyn Any>;
    fn update(&self, old: &dyn Erased<M>, state: &mut dyn Any) -> Dirty;
    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size;
    fn baseline(&self, state: &dyn Any) -> Option<f32>;
    fn paint(&self, state: &dyn Any, cx: &mut PaintCx);
    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx);
    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool;
    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool;
    fn animating(&self, state: &dyn Any) -> bool;
    fn access(&self, state: &dyn Any, info: &mut AccessInfo);
    fn focusable(&self) -> Focus;
    fn cursor(&self) -> Option<CursorIcon>;
}

fn state_ref<S: 'static>(state: &dyn Any) -> &S {
    state.downcast_ref().expect("a node keeps the state its widget created")
}

fn state_mut<S: 'static>(state: &mut dyn Any) -> &mut S {
    state.downcast_mut().expect("a node keeps the state its widget created")
}

/// A widget as the runtime holds it.
pub(crate) struct Plain<W>(pub W);

impl<M: 'static, W: Widget<M>> Erased<M> for Plain<W> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn widget_type(&self) -> TypeId {
        TypeId::of::<W>()
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(Widget::<M>::create_state(&self.0))
    }

    fn update(&self, old: &dyn Erased<M>, state: &mut dyn Any) -> Dirty {
        match old.as_any().downcast_ref::<Plain<W>>() {
            Some(old) => Widget::<M>::update(&self.0, &old.0, state_mut(state)),
            None => Dirty::Layout,
        }
    }

    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        Widget::<M>::measure(&self.0, state_mut(state), env, known, avail)
    }

    fn baseline(&self, state: &dyn Any) -> Option<f32> {
        Widget::<M>::baseline(&self.0, state_ref(state))
    }

    fn paint(&self, state: &dyn Any, cx: &mut PaintCx) {
        Widget::<M>::paint(&self.0, state_ref(state), cx)
    }

    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx) {
        Widget::<M>::paint_over(&self.0, state_ref(state), cx)
    }

    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        Widget::<M>::event(&self.0, state_mut(state), cx, event)
    }

    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool {
        Widget::<M>::tick(&self.0, state_mut(state), cx, dt)
    }

    fn animating(&self, state: &dyn Any) -> bool {
        Widget::<M>::animating(&self.0, state_ref(state))
    }

    fn access(&self, state: &dyn Any, info: &mut AccessInfo) {
        Widget::<M>::access(&self.0, state_ref(state), info)
    }

    fn focusable(&self) -> Focus {
        Widget::<M>::focusable(&self.0)
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Widget::<M>::cursor(&self.0)
    }
}

/// A widget whose messages are turned into another type: what [`Element::map`](crate::Element::map)
/// wraps each widget in, so a component written for its own message type sits inside a bigger app.
pub(crate) struct Mapped<A: 'static, B: 'static> {
    pub inner: Box<dyn Erased<A>>,
    pub f: Rc<dyn Fn(A) -> B>,
}

impl<A: 'static, B: 'static> Erased<B> for Mapped<A, B> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// The wrapped widget's type: state is kept across a change of mapping, as it is the same widget.
    fn widget_type(&self) -> TypeId {
        self.inner.widget_type()
    }

    fn create_state(&self) -> Box<dyn Any> {
        self.inner.create_state()
    }

    fn update(&self, old: &dyn Erased<B>, state: &mut dyn Any) -> Dirty {
        match old.as_any().downcast_ref::<Mapped<A, B>>() {
            Some(old) => self.inner.update(&*old.inner, state),
            None => Dirty::Layout,
        }
    }

    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        self.inner.measure(state, env, known, avail)
    }

    fn baseline(&self, state: &dyn Any) -> Option<f32> {
        self.inner.baseline(state)
    }

    fn paint(&self, state: &dyn Any, cx: &mut PaintCx) {
        self.inner.paint(state, cx)
    }

    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx) {
        self.inner.paint_over(state, cx)
    }

    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<B>, event: &WidgetEvent) -> bool {
        // The inner widget writes into an output of its own type; requests and effects pass through
        // unchanged and its messages come back mapped.
        let mut out = Output::<A> {
            messages: Vec::new(),
            effects: std::mem::take(&mut cx.out.effects),
            req: std::mem::take(&mut cx.out.req),
        };
        let used = {
            let mut inner = EventCx {
                env: cx.env,
                id: cx.id,
                rect: cx.rect,
                content: cx.content,
                hovered: cx.hovered,
                pressed: cx.pressed,
                focused: cx.focused,
                focus_visible: cx.focus_visible,
                window_focused: cx.window_focused,
                mods: cx.mods,
                time: cx.time,
                out: &mut out,
                clipboard: cx.clipboard,
            };
            self.inner.event(state, &mut inner, event)
        };
        cx.out.effects = out.effects;
        cx.out.req = out.req;
        cx.out.messages.extend(out.messages.into_iter().map(|m| (self.f)(m)));
        used
    }

    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool {
        self.inner.tick(state, cx, dt)
    }

    fn animating(&self, state: &dyn Any) -> bool {
        self.inner.animating(state)
    }

    fn access(&self, state: &dyn Any, info: &mut AccessInfo) {
        self.inner.access(state, info)
    }

    fn focusable(&self) -> Focus {
        self.inner.focusable()
    }

    fn cursor(&self) -> Option<CursorIcon> {
        self.inner.cursor()
    }
}
