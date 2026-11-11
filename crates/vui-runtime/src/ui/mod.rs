//! The UI: the retained tree plus everything that happens to it. The app hands over a new element tree
//! after each change, feeds in platform events and frame ticks, and gets back messages for its `update`,
//! effects for the window system, and a scene to render.

mod events;

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

    /// Installs the tree the app wants on screen. Nodes that match keep their state.
    pub fn set_view(&mut self, view: impl IntoElement<M>) {
        let el = view.into_element();
        self.set_view_inner(el);
        self.focus_arrivals();
    }

    /// Focus moves to an element marked `autofocus` that has just appeared, or into a focus trap
    /// that has just appeared and holds no focus yet.
    fn focus_arrivals(&mut self) {
        let Some(root) = self.root.as_ref() else { return };
        let mut showing = Vec::new();
        find_flagged(root, &mut showing);
        let before = std::mem::replace(&mut self.flagged, showing.iter().map(|(id, _)| *id).collect());
        let arrived = showing.into_iter().filter(|(id, _)| !before.contains(id));
        let (mut auto, mut trap) = (None, None);
        for (id, autofocus) in arrived {
            if autofocus {
                auto = auto.or(Some(id));
            } else {
                trap = Some(id);
            }
        }
        let target = auto.or_else(|| {
            let trap = trap?;
            let node = find_id(root, trap)?;
            if self.core.focus.is_some_and(|f| find_id(node, f).is_some()) {
                return None;
            }
            collect_stops(node).first().map(|s| s.entry)
        });
        if let Some(id) = target {
            self.sync();
            let keyboard = self.core.keyboard;
            if let Some(root) = self.root.as_mut() {
                Run { root, core: &mut self.core }.set_focus(Some(id), keyboard);
            }
        }
    }

    fn set_view_inner(&mut self, el: crate::element::Element<M>) {
        let core = &mut self.core;
        let dirty = match self.root.as_mut() {
            Some(root) if root.widget.widget_type() == el.widget.widget_type() => root.update(el, &mut core.ids, &core.env),
            _ => {
                self.root = Some(Node::build(el, &mut core.ids, &core.env));
                crate::widget::Dirty::Layout
            }
        };
        match dirty {
            crate::widget::Dirty::Layout => core.layout_dirty = true,
            crate::widget::Dirty::Paint => core.paint_dirty = true,
            crate::widget::Dirty::None => {}
        }
        // A transition or a widget animation begun by this update needs frames from here on.
        core.animating |= self.root.as_ref().is_some_and(Node::animating);
        self.forget_missing();
    }

    /// Drops every reference to nodes that are no longer in the tree.
    fn forget_missing(&mut self) {
        let core = &mut self.core;
        let Some(root) = self.root.as_ref() else { return };
        let mut live = HashSet::new();
        collect_ids(root, &mut live);
        if core.focus.is_some_and(|f| !live.contains(&f)) {
            core.focus = None;
            core.focus_visible = false;
            core.out.effects.push(Effect::ImeArea(None));
        }
        core.hover.retain(|id| live.contains(id));
        if core.press.as_ref().is_some_and(|p| !live.contains(&p.id)) {
            core.press = None;
        }
        if core.bar_drag.as_ref().is_some_and(|d| !live.contains(&d.id)) {
            core.bar_drag = None;
        }
        if core.drop_target.is_some_and(|d| !live.contains(&d)) {
            core.drop_target = None;
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
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

    /// Seconds until a widget or an app timer wants a `tick` even though nothing is animating (a
    /// blinking caret, `Cx::after`).
    pub fn next_wake(&self) -> Option<f32> {
        let timer = self.timers.iter().map(|(due, _)| *due).reduce(f32::min);
        let at = match (self.core.wake_at, timer) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        at.map(|t| (t - self.core.time).max(0.0))
    }

    /// Has background tasks and `AppSender`s call `waker` once they have sent something, so the
    /// backend wakes its event loop and calls [`Ui::poll`]. Without one, messages wait for the next
    /// `tick`.
    pub fn set_waker(&mut self, waker: Waker) {
        self.tasks.waker = Some(waker);
    }

    /// Hands the app what background tasks and `AppSender`s have sent so far.
    pub fn poll(&mut self) {
        let mut any = false;
        while let Ok(msg) = self.inbox.try_recv() {
            let pending = &self.tasks.pending;
            pending.set(pending.get().saturating_sub(1));
            self.core.out.messages.push(msg);
            any = true;
        }
        if any {
            self.sync();
            self.pump();
            self.sync();
        }
    }

    /// Background tasks (`Cx::spawn`) started and not yet answered.
    pub fn pending_tasks(&self) -> usize {
        self.tasks.pending.get()
    }

    /// Blocks until a background task or `AppSender` sends something, for at most `limit`, and
    /// hands it to the app. False if nothing came. For tests and tools.
    pub fn wait(&mut self, limit: std::time::Duration) -> bool {
        match self.inbox.recv_timeout(limit) {
            Ok(msg) => {
                let pending = &self.tasks.pending;
                pending.set(pending.get().saturating_sub(1));
                self.send(msg);
                self.poll();
                true
            }
            Err(_) => false,
        }
    }

    /// Hands the app the timers that are due.
    fn fire_timers(&mut self) {
        let now = self.core.time + TIMER_SLACK;
        if !self.timers.iter().any(|(due, _)| *due <= now) {
            return;
        }
        let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.timers).into_iter().partition(|(at, _)| *at <= now);
        self.timers = waiting;
        self.core.out.messages.extend(due.into_iter().map(|(_, msg)| msg));
        self.pump();
    }

    /// Seconds of UI time that have passed, as summed from `tick`.
    pub fn time(&self) -> f32 {
        self.core.time
    }

    /// Advances animations by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let core = &mut self.core;
        core.time += dt;
        let mut ts = TickState {
            env: &core.env,
            time: core.time,
            window_focused: core.window_focused,
            focus: core.focus,
            wake: None,
            moving: false,
            layout: false,
            arrange: false,
            paint: false,
        };
        if let Some(root) = self.root.as_mut() {
            root.tick(dt, &mut ts);
        }
        let (moving, wake, layout, arrange, painted) = (ts.moving, ts.wake, ts.layout, ts.arrange, ts.paint);
        core.animating = moving;
        let now = core.time;
        let mut at = wake.map(|s| now + s);
        if let Some(old) = core.wake_at
            && old > now
        {
            at = Some(at.map_or(old, |a| a.min(old)));
        }
        core.wake_at = at;
        core.layout_dirty |= layout;
        core.arrange_dirty |= arrange;
        core.paint_dirty |= painted;
        self.sync();
        self.fire_timers();
        self.poll();
    }

    /// Lays the tree out if anything changed, and keeps hover in step with whatever moved under the
    /// pointer.
    pub(crate) fn sync(&mut self) {
        for _ in 0..4 {
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
            layout_root(root, &mut core.env, core.size);
            lay_out_fixed(root, &mut core.env, core.size);
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
        }
        core.env.res.fonts.end_frame();
        core.paint_dirty = false;
        scene
    }

    /// The border box of a keyed element in window coordinates, after laying out if needed.
    pub fn rect_of(&mut self, key: impl Into<Key>) -> Option<Rect> {
        self.inspect(key).map(|i| i.rect)
    }

    /// What the runtime knows about a keyed element.
    pub fn inspect(&mut self, key: impl Into<Key>) -> Option<Inspect> {
        self.sync();
        let key = key.into();
        let root = self.root.as_ref()?;
        let node = find_key(root, &key)?;
        let core = &self.core;
        let focused = core.focus == Some(node.id);
        Some(Inspect {
            rect: node.rect,
            hovered: core.hover.contains(&node.id),
            pressed: core.press.as_ref().is_some_and(|p| p.id == node.id && p.inside),
            focused,
            focus_visible: focused && core.focus_visible,
            disabled: node.input.disabled,
            scroll: node.scroll.offset(),
            scroll_max: node.scroll.max,
            thumb_x: node.bar(Axis::X).map(|b| b.thumb),
            thumb_y: node.bar(Axis::Y).map(|b| b.thumb),
            bars: node.scroll.bars.value(),
        })
    }

    /// Moves focus to the keyed element, showing the ring if the last input came from the keyboard.
    /// Returns false when there is no such element or it cannot take focus.
    pub fn focus_key(&mut self, key: impl Into<Key>) -> bool {
        self.sync();
        let key = key.into();
        let Some(root) = self.root.as_mut() else { return false };
        let Some(node) = find_key(root, &key) else { return false };
        let id = node.id;
        let Some(path) = path_of(root, id) else { return false };
        if node.focus_mode() == Focus::Never || chain(root, &path).iter().any(|n| n.input.disabled || n.eff.hidden) {
            return false;
        }
        let mut run = Run { root, core: &mut self.core };
        run.set_focus(Some(id), run.core.keyboard);
        let rect = node_at(run.root, &path).rect;
        run.reveal(&path, rect);
        self.sync();
        true
    }

    /// True when a press at `p` should move the window: it lands in an element marked
    /// [`window_drag`](crate::Builder::window_drag) and on nothing inside it that takes clicks or focus.
    pub fn drags_window(&mut self, p: Point) -> bool {
        self.sync();
        let Some(root) = self.root.as_ref() else { return false };
        let Some(path) = hit(root, p) else { return false };
        let mut node = root;
        let mut drag = node.input.window_drag;
        for &i in &path {
            node = &node.children[i];
            if node.input.clickable() || node.input.drop.is_some() || node.focus_mode() != Focus::Never || node.eff.overflow == Overflow::Scroll {
                drag = false;
            } else if node.input.window_drag {
                drag = true;
            }
        }
        drag
    }

    /// Whether something at `p` takes the pointer: an element that is not marked
    /// [`pointer_events(false)`](crate::Builder::pointer_events), the root included.
    pub fn takes_pointer(&mut self, p: Point) -> bool {
        self.sync();
        self.root.as_ref().is_some_and(|root| hit(root, p).is_some())
    }

    /// The key of the focused element, if it has one.
    pub fn focused_key(&self) -> Option<Key> {
        let (root, id) = (self.root.as_ref()?, self.core.focus?);
        find_id(root, id).and_then(|n| n.key.clone())
    }

    /// How many nodes the tree has.
    pub fn node_count(&self) -> usize {
        fn count<M: 'static>(n: &Node<M>) -> usize {
            1 + n.children.iter().map(count).sum::<usize>()
        }
        self.root.as_ref().map_or(0, count)
    }
}

fn collect_ids<M: 'static>(node: &Node<M>, out: &mut HashSet<NodeId>) {
    out.insert(node.id);
    for child in &node.children {
        collect_ids(child, out);
    }
}

fn clear_caches<M: 'static>(node: &mut Node<M>) {
    node.cache.clear();
    for child in &mut node.children {
        clear_caches(child);
    }
}

pub(crate) fn find_id<M: 'static>(node: &Node<M>, id: NodeId) -> Option<&Node<M>> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_id(c, id))
}

fn find_key<'a, M: 'static>(node: &'a Node<M>, key: &Key) -> Option<&'a Node<M>> {
    if node.key.as_ref() == Some(key) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_key(c, key))
}

/// Lays out every fixed element against the window, outer ones first.
fn lay_out_fixed<M: 'static>(node: &mut Node<M>, env: &mut Env, window: Size) {
    for child in &mut node.children {
        if child.eff.hidden {
            continue;
        }
        if child.eff.position == Position::Fixed {
            place_fixed(child, env, window);
        }
        lay_out_fixed(child, env, window);
    }
}

/// The showing elements marked `autofocus` (true) or `trap_focus` (false), in tree order.
fn find_flagged<M: 'static>(node: &Node<M>, out: &mut Vec<(NodeId, bool)>) {
    if node.eff.hidden {
        return;
    }
    if node.input.autofocus {
        out.push((node.id, true));
    } else if node.input.trap_focus {
        out.push((node.id, false));
    }
    for child in &node.children {
        find_flagged(child, out);
    }
}

/// Paths to the elements that close on an outside press or Escape (`on_dismiss`), in the order they
/// are drawn: the last is on top.
pub(crate) fn dismissible<M: 'static>(root: &Node<M>) -> Vec<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, depth: usize, path: &mut Vec<usize>, out: &mut Vec<(usize, Vec<usize>)>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            let depth = depth + usize::from(child.eff.float);
            if child.input.dismiss.is_some() {
                out.push((depth, path.clone()));
            }
            walk(child, depth, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut Vec::new(), &mut out);
    out.sort_by_key(|(depth, _)| *depth);
    out.into_iter().map(|(_, p)| p).collect()
}

/// The innermost focus trap that shows, if any: Tab stays inside it.
pub(crate) fn focus_trap<M: 'static>(root: &Node<M>) -> Option<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, path: &mut Vec<usize>, best: &mut Option<Vec<usize>>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            if child.input.trap_focus {
                *best = Some(path.clone());
            }
            walk(child, path, best);
            path.pop();
        }
    }
    let mut best = None;
    walk(root, &mut Vec::new(), &mut best);
    best
}

/// The child indices down to the topmost node that takes pointer input at `p`.
fn hit<M: 'static>(root: &Node<M>, p: Point) -> Option<Vec<usize>> {
    /// Paths to the floating elements, each with how many floats it is inside of.
    fn floats<M: 'static>(node: &Node<M>, depth: usize, path: &mut Vec<usize>, out: &mut Vec<(usize, Vec<usize>)>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            if child.eff.float {
                out.push((depth, path.clone()));
            }
            floats(child, depth + usize::from(child.eff.float), path, out);
            path.pop();
        }
    }
    fn walk<M: 'static>(node: &Node<M>, p: Point, path: &mut Vec<usize>) -> bool {
        if node.eff.hidden || node.clip.is_some_and(|c| !c.contains(p)) {
            return false;
        }
        if node.eff.overflow != Overflow::Visible && !node.rect.contains(p) {
            return false;
        }
        for (i, child) in node.children.iter().enumerate().rev() {
            if child.eff.float {
                continue;
            }
            path.push(i);
            if walk(child, p, path) {
                return true;
            }
            path.pop();
        }
        node.input.pointer_events && node.rect.contains(p)
    }
    let mut above = Vec::new();
    floats(root, 0, &mut Vec::new(), &mut above);
    // Floats paint in tree order, and the floats inside them after all of those: the reverse of
    // that is the order they are hit in.
    above.sort_by_key(|(depth, _)| *depth);
    for (_, mut path) in above.into_iter().rev() {
        let node = path.iter().fold(root, |n, &i| &n.children[i]);
        if walk(node, p, &mut path) {
            return Some(path);
        }
    }
    let mut path = Vec::new();
    walk(root, p, &mut path).then_some(path)
}

/// One place Tab stops: a lone focusable element, or a whole focus group.
struct Stop {
    /// Where Tab lands.
    entry: NodeId,
    /// 0 for the first item, 1 for the item that had focus last, 2 for the item marked `tab_default`.
    rank: u8,
    /// Everything that counts as being on this stop.
    members: Vec<NodeId>,
}

/// The Tab order. Every focusable element is a stop of its own, except the items of a focus group,
/// which share one stop where the group's first item is.
fn collect_stops<M: 'static>(root: &Node<M>) -> Vec<Stop> {
    struct Walk {
        stops: Vec<Stop>,
        /// Group node to its stop.
        groups: HashMap<NodeId, usize>,
    }
    /// `group` is the nearest group above `node` and the item it last had focus on.
    fn walk<M: 'static>(node: &Node<M>, group: Option<(NodeId, Option<NodeId>)>, w: &mut Walk) {
        if node.eff.hidden || node.input.disabled {
            return;
        }
        if node.focus_mode() != Focus::Never {
            match group {
                None => w.stops.push(Stop { entry: node.id, rank: 0, members: vec![node.id] }),
                Some((g, memory)) => {
                    let i = match w.groups.get(&g) {
                        Some(&i) => i,
                        None => {
                            w.stops.push(Stop { entry: node.id, rank: 0, members: Vec::new() });
                            w.groups.insert(g, w.stops.len() - 1);
                            w.stops.len() - 1
                        }
                    };
                    let rank = if node.input.tab_default {
                        2
                    } else if memory == Some(node.id) {
                        1
                    } else {
                        0
                    };
                    let stop = &mut w.stops[i];
                    stop.members.push(node.id);
                    if rank > stop.rank {
                        stop.entry = node.id;
                        stop.rank = rank;
                    }
                }
            }
        }
        // A group's own focus belongs to the context around it; what is inside belongs to the group.
        let inner = if node.input.group.is_some() { Some((node.id, node.memory)) } else { group };
        for child in &node.children {
            walk(child, inner, w);
        }
    }
    let mut w = Walk { stops: Vec::new(), groups: HashMap::new() };
    walk(root, None, &mut w);
    w.stops
}

/// The focusable elements that belong to this focus group, in order: those inside it that are not
/// inside a group of their own.
fn group_items<M: 'static>(group: &Node<M>) -> Vec<NodeId> {
    fn walk<M: 'static>(node: &Node<M>, out: &mut Vec<NodeId>) {
        if node.eff.hidden || node.input.disabled {
            return;
        }
        if node.focus_mode() != Focus::Never {
            out.push(node.id);
        }
        if node.input.group.is_none() {
            for child in &node.children {
                walk(child, out);
            }
        }
    }
    let mut out = Vec::new();
    for child in &group.children {
        walk(child, &mut out);
    }
    out
}

/// Hands one event to one widget and returns whether it used it.
fn call_widget<M: 'static>(core: &mut Core<M>, node: &mut Node<M>, event: &WidgetEvent) -> bool {
    let focused = core.focus == Some(node.id);
    let mut cx = EventCx {
        env: &core.env,
        id: node.id,
        rect: node.rect,
        content: node.content_rect(node.rect),
        hovered: core.hover.contains(&node.id),
        pressed: core.press.as_ref().is_some_and(|p| p.id == node.id && p.inside),
        focused,
        focus_visible: focused && core.focus_visible,
        window_focused: core.window_focused,
        mods: core.mods,
        time: core.time,
        out: &mut core.out,
        clipboard: &*core.clipboard,
    };
    node.widget.event(&mut *node.state, &mut cx, event)
}

/// The tree and the core together, so event handling can reach both at once.
pub(crate) struct Run<'a, M: 'static> {
    pub root: &'a mut Node<M>,
    pub core: &'a mut Core<M>,
}

/// The pointer shape a chain of nodes asks for, innermost first. A disabled node stops the search.
fn cursor_of<M: 'static>(nodes: &[&Node<M>]) -> Option<CursorIcon> {
    for node in nodes.iter().rev() {
        if node.input.disabled {
            return Some(CursorIcon::Arrow);
        }
        if let Some(c) = node.input.cursor.or_else(|| node.widget.cursor()) {
            return Some(c);
        }
    }
    None
}
