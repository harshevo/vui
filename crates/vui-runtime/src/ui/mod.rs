//! The UI: the retained tree plus everything that happens to it. The app hands over a new element tree
//! after each change, feeds in platform events and frame ticks, and gets back messages for its `update`,
//! effects for the window system, and a scene to render.

mod events;
mod tooltip;
mod view;

pub(crate) use query::find_id;
pub(crate) use tree::*;
use view::*;
mod clock;
mod query;
mod tree;

use std::collections::{HashMap, HashSet};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};

use vui_core::{
    Color, CursorIcon, Event, ImeEvent, KeyEvent, Mode, Modifiers, NamedKey, Point, PointerButton, Rect, Resources, Size,
    Theme, Vec2, vec2,
};

use crate::app::{App, Cx, Driver, Tasks, Waker};
use crate::effects::{Clipboard, Effect, MemoryClipboard};
use crate::element::{Click, IntoElement};
use crate::key::{Key, NodeId};
use crate::layout::{layout_root, place_fixed};
use crate::node::{Ids, Node, TickState, chain, node_at, node_at_mut, path_of};
use crate::paint::{PaintEnv, paint};
use crate::style::{Axis, Overflow, Position};
use crate::widget::{Env, EventCx, Focus, FocusBy, Output, WidgetEvent};
use vui_core::Scene;

/// The pointer is held down on a node.
struct Press {
    id: NodeId,
    button: PointerButton,
    count: u8,
    /// The widget took the press for itself, so there is no click to send.
    widget: bool,
    /// The pointer is over the node right now. Letting go outside cancels the click.
    inside: bool,
}

/// A scrollbar thumb being dragged.
struct BarDrag {
    id: NodeId,
    axis: Axis,
    /// Where on the thumb the pointer took hold, along the axis.
    grab: f32,
}

pub(crate) struct Core<M: 'static> {
    pub env: Env,
    pub size: Size,
    ids: Ids,
    time: f32,
    mods: Modifiers,
    pointer: Option<Point>,
    window_focused: bool,
    pub focus: Option<NodeId>,
    focus_visible: bool,
    /// The last input came from the keyboard, so focus that follows shows its ring.
    keyboard: bool,
    hover: Vec<NodeId>,
    press: Option<Press>,
    bar_drag: Option<BarDrag>,
    drop_target: Option<NodeId>,
    cursor: CursorIcon,
    out: Output<M>,
    clipboard: Rc<dyn Clipboard>,
    layout_dirty: bool,
    arrange_dirty: bool,
    paint_dirty: bool,
    animating: bool,
    wake_at: Option<f32>,
    tip: tooltip::Tip,
}

impl<M: 'static> Core<M> {
    fn wake(&mut self, seconds: f32) {
        let at = self.time + seconds.max(0.0);
        self.wake_at = Some(self.wake_at.map_or(at, |w| w.min(at)));
    }
}

/// What the runtime knows about one keyed element. Meant for tests and tools.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspect {
    /// Border box in window coordinates.
    pub rect: Rect,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub focus_visible: bool,
    pub disabled: bool,
    /// How far a scroll container has scrolled.
    pub scroll: Vec2,
    /// How far it can scroll.
    pub scroll_max: Vec2,
    /// The scrollbar thumbs, if the element scrolls that way, and how visible the bars are (0 to 1).
    pub thumb_x: Option<Rect>,
    pub thumb_y: Option<Rect>,
    pub bars: f32,
}

pub struct Ui<M: 'static> {
    pub(crate) root: Option<Node<M>>,
    pub(crate) core: Core<M>,
    /// The app this UI runs, if it hosts one: it answers messages and supplies the view.
    driver: Option<Box<dyn Driver<M>>>,
    /// Light or dark, as the system has it. Hosted apps are told when it changes.
    system: Mode,
    quit: bool,
    pass_clicks: Option<bool>,
    take_keys: Option<bool>,
    /// Clear to nothing instead of the theme's background: for overlays the desktop shows through.
    transparent: bool,
    /// Where background tasks and `AppSender`s deliver, and what they send it on.
    tasks: Tasks<M>,
    inbox: Receiver<M>,
    /// The `autofocus` and `trap_focus` elements showing after the last view: focus moves only
    /// into ones that were not.
    flagged: HashSet<NodeId>,
    /// Messages waiting for their time (`Cx::after`), as (UI time due, message), in the order set.
    timers: Vec<(f32, M)>,
}

/// A timer counts as due this close to its time, so a clock summed from frame steps still fires it.
const TIMER_SLACK: f32 = 1e-4;

/// How many times in a row the hosted app may answer messages that its own answers produced.
const MAX_PUMP_ROUNDS: usize = 64;

impl<M: 'static> Ui<M> {
    pub fn new(res: Resources, theme: Theme, size: Size, scale: f32) -> Ui<M> {
        res.fonts.set_families(theme.font_sans, theme.font_mono);
        let (tx, inbox) = mpsc::channel();
        Ui {
            root: None,
            core: Core {
                env: Env { window: size.w, ..Env::new(res, theme, scale) },
                size,
                ids: Ids::default(),
                time: 0.0,
                mods: Modifiers::NONE,
                pointer: None,
                window_focused: true,
                focus: None,
                focus_visible: false,
                keyboard: false,
                hover: Vec::new(),
                press: None,
                bar_drag: None,
                drop_target: None,
                cursor: CursorIcon::Arrow,
                out: Output::default(),
                clipboard: Rc::new(MemoryClipboard::default()),
                layout_dirty: true,
                arrange_dirty: true,
                paint_dirty: true,
                animating: false,
                wake_at: None,
                tip: tooltip::Tip::default(),
            },
            driver: None,
            system: Mode::Light,
            quit: false,
            pass_clicks: None,
            take_keys: None,
            transparent: false,
            tasks: Tasks { tx, waker: None, pending: Rc::new(Cell::new(0)) },
            inbox,
            flagged: HashSet::new(),
            timers: Vec::new(),
        }
    }

    /// A UI that runs `app`: events become messages, the app's `update` answers them, and the window
    /// shows its `view` and `theme`, all inside `handle`. `take_messages` stays empty.
    pub fn with_app<A: App<Msg = M>>(res: Resources, size: Size, scale: f32, app: A) -> Ui<M> {
        let mut ui = Ui::new(res, app.theme(Mode::Light), size, scale);
        let driver: Box<dyn Driver<M>> = Box::new(app);
        ui.show(&*driver);
        let first = driver.resized(size);
        ui.driver = Some(driver);
        if let Some(message) = first {
            ui.send(message);
        }
        ui
    }

    /// Puts the app's current view and theme on screen.
    fn show(&mut self, driver: &dyn Driver<M>) {
        self.set_view(driver.view());
        let theme = driver.theme(self.system);
        if theme != *self.core.env.theme {
            self.set_theme(theme);
        }
    }

    /// Hands the hosted app the messages widgets have sent and shows its new view, until nothing more
    /// comes back.
    pub(crate) fn pump(&mut self) {
        let Some(mut driver) = self.driver.take() else { return };
        for _ in 0..MAX_PUMP_ROUNDS {
            // Laying out a new view can move things under the pointer, which sends hover messages.
            self.sync();
            let messages = self.take_messages();
            if messages.is_empty() {
                break;
            }
            let mut cx = Cx::new(self.tasks.clone());
            for message in messages {
                driver.update(message, &mut cx);
            }
            self.show(&*driver);
            let now = self.core.time;
            self.timers.extend(cx.timers.into_iter().map(|(delay, msg)| (now + delay.max(0.0), msg)));
            self.core.out.messages.extend(cx.then);
            self.quit |= cx.quit;
            self.pass_clicks = cx.pass_clicks.or(self.pass_clicks);
            self.take_keys = cx.take_keys.or(self.take_keys);
            if let Some(key) = cx.focus {
                self.focus_key(key);
            }
        }
        self.driver = Some(driver);
    }

    /// The hosted app, if there is one and it is an `A`. For tests and tools.
    pub fn app<A: App<Msg = M>>(&self) -> Option<&A> {
        self.driver.as_ref()?.as_any().downcast_ref()
    }

    /// Changes the hosted app's state directly, then shows its new view: for the host to report
    /// things that are not widget messages, such as the system switching to dark mode.
    pub fn update_app<A: App<Msg = M>, R>(&mut self, f: impl FnOnce(&mut A) -> R) -> Option<R> {
        let mut driver = self.driver.take()?;
        let out = driver.as_any_mut().downcast_mut::<A>().map(f);
        self.show(&*driver);
        self.driver = Some(driver);
        self.sync();
        out
    }

    /// Tells the UI whether the system is in light or dark mode. A hosted app gets asked for its theme
    /// again.
    pub fn set_system_mode(&mut self, mode: Mode) {
        if mode == self.system {
            return;
        }
        self.system = mode;
        if let Some(driver) = self.driver.take() {
            self.show(&*driver);
            self.driver = Some(driver);
            self.sync();
        }
    }

    pub fn system_mode(&self) -> Mode {
        self.system
    }

    /// Hands the hosted app a message from outside the widgets: a hotkey, a timer, another thread.
    pub fn send(&mut self, msg: M) {
        self.sync();
        self.core.out.messages.push(msg);
        self.pump();
        self.sync();
    }

    /// Leaves everything the views do not paint see-through, for windows that float over the desktop.
    pub fn set_transparent(&mut self, transparent: bool) {
        if transparent != self.transparent {
            self.transparent = transparent;
            self.core.paint_dirty = true;
        }
    }

    pub fn transparent(&self) -> bool {
        self.transparent
    }

    /// Whether the app asked for clicks to pass through the window (`Cx::pass_clicks`); None until it
    /// says, when the window keeps its own setting.
    pub fn passes_clicks(&self) -> Option<bool> {
        self.pass_clicks
    }

    /// Whether the app asked for the keyboard (`Cx::take_keys`); None until it says.
    pub fn takes_keys(&self) -> Option<bool> {
        self.take_keys
    }

    /// The app asked to close (`Cx::quit`).
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.core.env.res.fonts.set_families(theme.font_sans, theme.font_mono);
        self.core.env.theme = Rc::new(theme);
        if let Some(root) = self.root.as_mut() {
            clear_caches(root);
        }
        self.core.layout_dirty = true;
        self.core.paint_dirty = true;
    }

    pub fn theme(&self) -> &Theme {
        &self.core.env.theme
    }

    /// The window's content size in logical pixels.
    pub fn set_size(&mut self, size: Size) {
        if size != self.core.size {
            self.core.size = size;
            self.core.layout_dirty = true;
            if size.w != self.core.env.window {
                self.core.env.window = size.w;
                // Responsive rules follow the window; nodes whose style changed lose their layout.
                if let Some(root) = self.root.as_mut()
                    && root.respond(size.w)
                {
                    self.core.animating |= root.animating();
                }
            }
            // The app hears of the new size, and may answer with a message to show it.
            let message = self.driver.as_ref().and_then(|d| d.resized(size));
            if let Some(message) = message {
                self.send(message);
            }
        }
    }

    pub fn size(&self) -> Size {
        self.core.size
    }

    /// Device pixels per logical pixel.
    pub fn set_scale(&mut self, scale: f32) {
        if scale != self.core.env.scale {
            self.core.env.scale = scale;
            self.core.paint_dirty = true;
        }
    }

    pub fn scale(&self) -> f32 {
        self.core.env.scale
    }

    /// Where Copy and Paste go. The backend installs the system pasteboard.
    pub fn set_clipboard(&mut self, clipboard: Rc<dyn Clipboard>) {
        self.core.clipboard = clipboard;
    }

    pub fn resources(&self) -> &Resources {
        &self.core.env.res
    }

    /// Messages widgets and handlers have produced since the last call, in order.
    pub fn take_messages(&mut self) -> Vec<M> {
        std::mem::take(&mut self.core.out.messages)
    }

    /// What the window system should do, in order.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.core.out.effects)
    }

    /// The window needs redrawing.
    pub fn needs_paint(&self) -> bool {
        self.core.paint_dirty || self.core.layout_dirty || self.core.arrange_dirty || self.core.animating
    }

    /// Something is animating: call `tick` and `paint` every frame until this turns false.
    pub fn needs_frame(&self) -> bool {
        self.core.animating
    }

    /// Lays the tree out if anything changed, and keeps hover in step with whatever moved under the
    /// pointer.
    pub(crate) fn sync(&mut self) {
        for _ in 0..4 {
            // Virtual lists build the rows that the last layout or scroll brought into view.
            if let Some(root) = self.root.as_mut()
                && root.refill_lazy(&mut self.core.ids, &self.core.env)
            {
                self.core.layout_dirty = true;
            }
            if !self.ensure_layout() {
                break;
            }
        }
    }

    /// Returns true if positions changed.
    fn ensure_layout(&mut self) -> bool {
        let Some(root) = self.root.as_mut() else { return false };
        let core = &mut self.core;
        if core.layout_dirty {
            // Container rules are matched against the layout they are in, so a rule that starts or
            // stops matching lays the tree out again, in the same frame. Three rounds settle any
            // sensible set of rules; one that keeps flipping is left where the third round put it.
            for _ in 0..3 {
                layout_root(root, &mut core.env, core.size);
                lay_out_fixed(root, &mut core.env, core.size);
                if !root.respond_rooms(core.env.window) {
                    break;
                }
            }
            core.layout_dirty = false;
            core.arrange_dirty = true;
        }
        if !core.arrange_dirty {
            return false;
        }
        root.arrange(Point::ZERO, Some(Rect::from_origin_size(Point::ZERO, core.size)));
        core.arrange_dirty = false;
        core.paint_dirty = true;
        if let Some(pos) = core.pointer {
            Run { root, core }.update_hover(Some(pos));
        }
        true
    }

    /// Feeds one platform event in.
    pub fn handle(&mut self, event: Event) {
        self.sync();
        match self.root.as_mut() {
            Some(root) => Run { root, core: &mut self.core }.event(event),
            None => {
                if let Event::WindowFocus(focused) = event {
                    self.core.window_focused = focused;
                }
            }
        }
        self.pump();
        self.sync();
    }

    /// Draws the tree.
    pub fn paint(&mut self) -> Scene {
        self.sync();
        let core = &mut self.core;
        let clear = if self.transparent { Color::TRANSPARENT } else { core.env.theme.palette.bg };
        let mut scene = Scene::new(core.size, core.env.scale).with_clear(clear);
        if let Some(root) = self.root.as_ref() {
            let pe = PaintEnv { env: &core.env, focus: core.focus, window_focused: core.window_focused, time: core.time };
            paint(root, &mut scene, &pe, 1.0);
            tooltip::paint(&core.tip, root, &core.env, core.size, &mut scene);
        }
        core.env.res.fonts.end_frame();
        core.paint_dirty = false;
        scene
    }
}
