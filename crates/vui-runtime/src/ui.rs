//! The UI: the retained tree plus everything that happens to it. The app hands over a new element tree
//! after each change, feeds in platform events and frame ticks, and gets back messages for its `update`,
//! effects for the window system, and a scene to render.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use vui_core::{
    CursorIcon, Event, ImeEvent, KeyEvent, Mode, Modifiers, NamedKey, Point, PointerButton, Rect, Resources, Size,
    Theme, Vec2, vec2,
};

use crate::app::{App, Cx, Driver};
use crate::effects::{Clipboard, Effect, MemoryClipboard};
use crate::element::{Click, IntoElement};
use crate::key::{Key, NodeId};
use crate::layout::layout_root;
use crate::node::{Ids, Node, TickState, chain, node_at, node_at_mut, path_of};
use crate::paint::{PaintEnv, paint};
use crate::style::{Axis, Overflow};
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
}

/// How many times in a row the hosted app may answer messages that its own answers produced.
const MAX_PUMP_ROUNDS: usize = 64;

impl<M: 'static> Ui<M> {
    pub fn new(res: Resources, theme: Theme, size: Size, scale: f32) -> Ui<M> {
        Ui {
            root: None,
            core: Core {
                env: Env::new(res, theme, scale),
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
        }
    }

    /// A UI that runs `app`: events become messages, the app's `update` answers them, and the window
    /// shows its `view` and `theme`, all inside `handle`. `take_messages` stays empty.
    pub fn with_app<A: App<Msg = M>>(res: Resources, size: Size, scale: f32, app: A) -> Ui<M> {
        let mut ui = Ui::new(res, app.theme(Mode::Light), size, scale);
        let driver: Box<dyn Driver<M>> = Box::new(app);
        ui.show(&*driver);
        ui.driver = Some(driver);
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
            let mut cx = Cx::default();
            for message in messages {
                driver.update(message, &mut cx);
            }
            self.show(&*driver);
            self.quit |= cx.quit;
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

    /// The app asked to close (`Cx::quit`).
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Installs the tree the app wants on screen. Nodes that match keep their state.
    pub fn set_view(&mut self, view: impl IntoElement<M>) {
        let el = view.into_element();
        let core = &mut self.core;
        let dirty = match self.root.as_mut() {
            Some(root) if root.widget.widget_type() == el.widget.widget_type() => root.update(el, &mut core.ids, &core.env),
            _ => {
                self.root = Some(Node::build(el, &mut core.ids));
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

    /// Seconds until a widget wants a `tick` even though nothing is animating (a blinking caret).
    pub fn next_wake(&self) -> Option<f32> {
        self.core.wake_at.map(|t| (t - self.core.time).max(0.0))
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
        let mut scene = Scene::new(core.size, core.env.scale).with_clear(core.env.theme.palette.bg);
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

/// The child indices down to the topmost node that takes pointer input at `p`.
fn hit<M: 'static>(root: &Node<M>, p: Point) -> Option<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, p: Point, path: &mut Vec<usize>) -> bool {
        if node.eff.hidden || node.clip.is_some_and(|c| !c.contains(p)) {
            return false;
        }
        if node.eff.overflow != Overflow::Visible && !node.rect.contains(p) {
            return false;
        }
        for (i, child) in node.children.iter().enumerate().rev() {
            path.push(i);
            if walk(child, p, path) {
                return true;
            }
            path.pop();
        }
        node.input.pointer_events && node.rect.contains(p)
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

impl<M: 'static> Run<'_, M> {
    fn event(&mut self, event: Event) {
        match &event {
            Event::PointerMoved { pos, mods } => self.pointer_moved(*pos, *mods),
            Event::PointerDown { pos, button, mods, click_count } => self.pointer_down(&event, *pos, *button, *mods, *click_count),
            Event::PointerUp { pos, button, mods } => self.pointer_up(&event, *pos, *button, *mods),
            Event::PointerLeft => self.pointer_left(),
            Event::Scroll { pos, delta, precise, .. } => self.scroll(&event, *pos, *delta, *precise),
            Event::Magnify { pos, .. } => {
                if let Some(path) = hit(self.root, *pos) {
                    self.deliver(&path, &WidgetEvent::Input(&event));
                }
            }
            Event::Key(key) => self.key(&event, key),
            Event::Ime(ime) => self.ime(&event, ime),
            Event::WindowFocus(focused) => self.window_focus(*focused),
            Event::DragOver { pos, .. } => {
                let target = self.drop_target_at(*pos);
                self.set_drop_target(target);
            }
            Event::DragLeft => self.set_drop_target(None),
            Event::Drop { pos, paths } => self.dropped(*pos, paths),
        }
    }

    // ---- delivering to widgets ----------------------------------------------------------------

    /// Hands `event` to the widget at `path`, then acts on what it asked for.
    pub fn call(&mut self, path: &[usize], event: &WidgetEvent) -> bool {
        let node = node_at_mut(self.root, path);
        let id = node.id;
        let handled = call_widget(self.core, node, event);
        self.after_call(path, id);
        handled
    }

    /// Offers `event` to the widgets from the leaf at `path` up to the root until one uses it.
    fn deliver(&mut self, path: &[usize], event: &WidgetEvent) -> bool {
        (0..=path.len()).rev().any(|k| self.call(&path[..k], event))
    }

    fn after_call(&mut self, path: &[usize], id: NodeId) {
        let req = std::mem::take(&mut self.core.out.req);
        if req.layout {
            self.invalidate(path);
        }
        if req.paint {
            self.core.paint_dirty = true;
        }
        if req.frame {
            self.core.animating = true;
        }
        if let Some(seconds) = req.wake {
            self.core.wake(seconds);
        }
        match req.capture {
            Some(true) if self.core.press.is_none() => {
                self.core.press =
                    Some(Press { id, button: PointerButton::Primary, count: 1, widget: true, inside: true });
            }
            Some(false) if self.core.press.as_ref().is_some_and(|p| p.id == id) => self.core.press = None,
            _ => {}
        }
        if let Some(rect) = req.reveal {
            self.reveal(path, rect);
        }
        match req.focus {
            Some(true) => self.set_focus_by(Some(id), self.core.keyboard, FocusBy::Other),
            Some(false) if self.core.focus == Some(id) => self.set_focus(None, false),
            _ => {}
        }
    }

    /// A node's size may have changed: forget what its ancestors measured.
    fn invalidate(&mut self, path: &[usize]) {
        for k in 0..=path.len() {
            node_at_mut(self.root, &path[..k]).cache.clear();
        }
        self.core.layout_dirty = true;
    }

    // ---- pointer ------------------------------------------------------------------------------

    fn disabled_on(&self, path: &[usize]) -> bool {
        chain(self.root, path).iter().any(|n| n.input.disabled)
    }

    fn pointer_moved(&mut self, pos: Point, mods: Modifiers) {
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        if self.drag_bar(pos) {
            return;
        }
        if let Some(press) = &self.core.press {
            let (id, widget) = (press.id, press.widget);
            if let Some(path) = path_of(self.root, id) {
                let node = node_at_mut(self.root, &path);
                let inside = node.hit_self(pos);
                node.anim.press.set_target(if inside { 1.0 } else { 0.0 });
                self.core.animating = true;
                if let Some(p) = self.core.press.as_mut() {
                    p.inside = inside;
                }
                if widget {
                    self.call(&path, &WidgetEvent::Input(&Event::PointerMoved { pos, mods }));
                }
            }
            self.update_hover(Some(pos));
            return;
        }
        if let Some(path) = self.update_hover(Some(pos)) {
            self.deliver(&path, &WidgetEvent::Input(&Event::PointerMoved { pos, mods }));
        }
    }

    fn pointer_left(&mut self) {
        self.core.pointer = None;
        if self.core.press.is_none() {
            self.update_hover(None);
        }
    }

    fn pointer_down(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers, count: u8) {
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        self.core.keyboard = false;
        if self.core.press.is_some() {
            // An earlier press never saw its release: end it before starting another.
            self.pointer_up(&Event::PointerUp { pos, button, mods }, pos, button, mods);
        }
        if button == PointerButton::Primary && self.grab_bar(pos) {
            return;
        }
        self.update_hover(Some(pos));
        let Some(path) = hit(self.root, pos) else {
            self.set_focus_by(None, false, FocusBy::Pointer);
            return;
        };
        if self.disabled_on(&path) {
            return;
        }
        self.focus_for_press(&path);

        let click = Click { pos, mods, button, count };
        for k in (0..=path.len()).rev() {
            let at = &path[..k];
            if self.call(at, &WidgetEvent::Input(event)) {
                let id = node_at(self.root, at).id;
                self.begin_press(at, Press { id, button, count, widget: true, inside: true });
                return;
            }
            let node = node_at(self.root, at);
            match button {
                PointerButton::Secondary => {
                    if let Some(h) = &node.input.secondary {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                        return;
                    }
                }
                PointerButton::Primary if node.input.clickable() => {
                    let id = node.id;
                    if let Some(h) = &node.input.press {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                    }
                    self.begin_press(at, Press { id, button, count, widget: false, inside: true });
                    return;
                }
                _ => {}
            }
        }
    }

    fn begin_press(&mut self, path: &[usize], press: Press) {
        node_at_mut(self.root, path).anim.press.set_target(1.0);
        self.core.press = Some(press);
        self.core.animating = true;
    }

    /// A press on a text field focuses it; a press on empty space gives focus up; a press on a button
    /// leaves focus where it was.
    fn focus_for_press(&mut self, path: &[usize]) {
        let nodes = chain(self.root, path);
        let modes: Vec<(NodeId, Focus)> = nodes.iter().rev().map(|n| (n.id, n.focus_mode())).collect();
        if let Some((id, _)) = modes.iter().find(|(_, f)| *f == Focus::Click) {
            let id = *id;
            self.set_focus_by(Some(id), false, FocusBy::Pointer);
        } else if modes.iter().all(|(_, f)| *f == Focus::Never) {
            self.set_focus_by(None, false, FocusBy::Pointer);
        }
    }

    fn pointer_up(&mut self, event: &Event, pos: Point, button: PointerButton, mods: Modifiers) {
        self.core.pointer = Some(pos);
        self.core.mods = mods;
        if button == PointerButton::Primary && self.core.bar_drag.take().is_some() {
            self.update_hover(Some(pos));
            return;
        }
        if self.core.press.as_ref().is_none_or(|p| p.button != button) {
            return;
        }
        let Some(press) = self.core.press.take() else { return };
        self.core.animating = true;
        if let Some(path) = path_of(self.root, press.id) {
            node_at_mut(self.root, &path).anim.press.set_target(0.0);
            if press.widget {
                self.call(&path, &WidgetEvent::Input(event));
            } else if node_at(self.root, &path).hit_self(pos) {
                let click = Click { pos, mods, button, count: press.count };
                let node = node_at(self.root, &path);
                if let Some(h) = &node.input.click {
                    let message = h(&click);
                    self.core.out.messages.push(message);
                }
                if press.count == 2 {
                    let node = node_at(self.root, &path);
                    if let Some(h) = &node.input.double_click {
                        let message = h(&click);
                        self.core.out.messages.push(message);
                    }
                }
            }
        }
        self.update_hover(Some(pos));
    }

    // ---- hover and cursor ---------------------------------------------------------------------

    /// Works out what the pointer is over, tells nodes that gained or lost it, and picks the cursor.
    /// Returns the path to the topmost node under the pointer.
    fn update_hover(&mut self, pos: Option<Point>) -> Option<Vec<usize>> {
        let path = pos.and_then(|p| hit(self.root, p));
        let mut ids: Vec<NodeId> = Vec::new();
        if let Some(path) = &path {
            for node in chain(self.root, path) {
                if node.input.disabled {
                    break;
                }
                ids.push(node.id);
            }
        }
        if let Some(press) = &self.core.press
            && !ids.contains(&press.id)
        {
            ids.clear();
        }

        let old = std::mem::replace(&mut self.core.hover, ids.clone());
        for id in old.iter().rev().filter(|id| !ids.contains(id)) {
            self.hover_changed(*id, false);
        }
        for id in ids.iter().filter(|id| !old.contains(id)) {
            self.hover_changed(*id, true);
        }

        let cursor = match (&self.core.press, &path) {
            (Some(press), _) => path_of(self.root, press.id).and_then(|p| cursor_of(&chain(self.root, &p))),
            (None, Some(path)) => cursor_of(&chain(self.root, path)),
            (None, None) => None,
        }
        .unwrap_or(CursorIcon::Arrow);
        if cursor != self.core.cursor {
            self.core.cursor = cursor;
            self.core.out.effects.push(Effect::Cursor(cursor));
        }
        self.update_bar_hover(pos, path.as_deref());
        path
    }

    fn hover_changed(&mut self, id: NodeId, on: bool) {
        let Some(path) = path_of(self.root, id) else { return };
        node_at_mut(self.root, &path).anim.hover.set_target(if on { 1.0 } else { 0.0 });
        self.core.animating = true;
        self.call(&path, &WidgetEvent::HoverChanged(on));
        let node = node_at(self.root, &path);
        if let Some(h) = &node.input.hover {
            let message = h(on);
            self.core.out.messages.push(message);
        }
    }

    // ---- scrolling ----------------------------------------------------------------------------

    fn scroll(&mut self, event: &Event, pos: Point, delta: Vec2, precise: bool) {
        self.core.pointer = Some(pos);
        let Some(path) = hit(self.root, pos) else { return };
        if self.disabled_on(&path) || self.deliver(&path, &WidgetEvent::Input(event)) {
            return;
        }
        for k in (0..=path.len()).rev() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let target = node.scroll.target();
            let max = node.scroll.max;
            let wanted = vec2((target.x - delta.x).clamp(0.0, max.x), (target.y - delta.y).clamp(0.0, max.y));
            let move_x = delta.x != 0.0 && (wanted.x - target.x).abs() > 0.01;
            let move_y = delta.y != 0.0 && (wanted.y - target.y).abs() > 0.01;
            if !move_x && !move_y {
                continue;
            }
            for (moves, axis, to) in [(move_x, Axis::X, wanted.x), (move_y, Axis::Y, wanted.y)] {
                if !moves {
                    continue;
                }
                let spring = node.scroll.spring_mut(axis);
                if precise {
                    spring.snap_to(to);
                } else {
                    spring.set_target(to);
                }
            }
            node.scroll.poke();
            self.core.arrange_dirty = true;
            self.core.animating = true;
            return;
        }
    }

    /// Scrolls the containers around the node at `path` until `target` (window coordinates) is in view.
    pub fn reveal(&mut self, path: &[usize], mut target: Rect) {
        const MARGIN: f32 = 8.0;
        for k in (0..=path.len()).rev() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let view = node.rect;
            let current = node.scroll.offset();
            let max = node.scroll.max;
            let margin_x = MARGIN.min(view.w / 2.0);
            let margin_y = MARGIN.min(view.h / 2.0);
            let shift = |lo: f32, hi: f32, view_lo: f32, view_hi: f32, margin: f32| {
                if lo < view_lo + margin {
                    lo - (view_lo + margin)
                } else if hi > view_hi - margin {
                    (hi - (view_hi - margin)).min(lo - (view_lo + margin))
                } else {
                    0.0
                }
            };
            let dx = shift(target.x, target.right(), view.x, view.right(), margin_x);
            let dy = shift(target.y, target.bottom(), view.y, view.bottom(), margin_y);
            let to = vec2((current.x + dx).clamp(0.0, max.x), (current.y + dy).clamp(0.0, max.y));
            if to.x != current.x {
                node.scroll.x.set_target(to.x);
            }
            if to.y != current.y {
                node.scroll.y.set_target(to.y);
            }
            node.scroll.poke();
            target.x -= to.x - current.x;
            target.y -= to.y - current.y;
            self.core.animating = true;
        }
    }

    // ---- scrollbars ---------------------------------------------------------------------------

    /// Starts dragging a scrollbar thumb under the pointer, if there is one.
    fn grab_bar(&mut self, pos: Point) -> bool {
        let Some(path) = hit(self.root, pos) else { return false };
        for k in (0..=path.len()).rev() {
            let node = node_at(self.root, &path[..k]);
            for axis in [Axis::Y, Axis::X] {
                let Some(bar) = node.bar(axis) else { continue };
                if node.scroll.bars.value() < 0.05 || !bar.thumb.inflate(3.0, 3.0).contains(pos) {
                    continue;
                }
                let grab = match axis {
                    Axis::X => pos.x - bar.thumb.x,
                    Axis::Y => pos.y - bar.thumb.y,
                };
                let id = node.id;
                self.core.bar_drag = Some(BarDrag { id, axis, grab });
                let node = node_at_mut(self.root, &path[..k]);
                node.scroll.thick.set_target(1.0);
                node.scroll.poke();
                self.core.animating = true;
                return true;
            }
        }
        false
    }

    /// Follows the pointer with a dragged thumb. Returns true if a drag is going on.
    fn drag_bar(&mut self, pos: Point) -> bool {
        let Some(drag) = &self.core.bar_drag else { return false };
        let (id, axis, grab) = (drag.id, drag.axis, drag.grab);
        let Some(path) = path_of(self.root, id) else {
            self.core.bar_drag = None;
            return false;
        };
        let node = node_at_mut(self.root, &path);
        if let Some(bar) = node.bar(axis)
            && bar.free > 0.0
        {
            let (start, at) = match axis {
                Axis::X => (node.rect.x + 2.0, pos.x),
                Axis::Y => (node.rect.y + 2.0, pos.y),
            };
            let along = (at - grab - start).clamp(0.0, bar.free);
            node.scroll.spring_mut(axis).snap_to(along / bar.free * bar.max);
            node.scroll.poke();
            self.core.arrange_dirty = true;
        }
        true
    }

    /// Fattens the bar under the pointer, and shows the bars of any scroll container it is over.
    fn update_bar_hover(&mut self, pos: Option<Point>, path: Option<&[usize]>) {
        let (Some(pos), Some(path)) = (pos, path) else { return };
        for k in 0..=path.len() {
            let node = node_at_mut(self.root, &path[..k]);
            if node.eff.overflow != Overflow::Scroll {
                continue;
            }
            let dragging = self.core.bar_drag.as_ref().is_some_and(|d| d.id == node.id);
            let near = [Axis::Y, Axis::X]
                .into_iter()
                .filter_map(|a| node.bar(a))
                .any(|b| b.thumb.inflate(6.0, 6.0).contains(pos));
            node.scroll.thick.set_target(if near || dragging { 1.0 } else { 0.0 });
            if near {
                node.scroll.poke();
            }
            self.core.animating = true;
        }
    }

    // ---- focus and keys -----------------------------------------------------------------------

    pub fn set_focus(&mut self, target: Option<NodeId>, visible: bool) {
        self.set_focus_by(target, visible, FocusBy::Other);
    }

    fn set_focus_by(&mut self, target: Option<NodeId>, visible: bool, by: FocusBy) {
        let visible = visible && target.is_some();
        let old = self.core.focus;
        self.core.focus_visible = visible;
        self.core.animating = true;
        self.core.paint_dirty = true;
        if old == target {
            if let Some(path) = target.and_then(|t| path_of(self.root, t)) {
                node_at_mut(self.root, &path).anim.focus.set_target(if visible { 1.0 } else { 0.0 });
            }
            return;
        }
        self.core.focus = target;
        self.remember(target);
        self.core.out.effects.push(Effect::ImeArea(None));
        if let Some(path) = old.and_then(|o| path_of(self.root, o)) {
            node_at_mut(self.root, &path).anim.focus.set_target(0.0);
            self.call(&path, &WidgetEvent::FocusChanged { focused: false, visible: false, by });
        }
        if let Some(path) = target.and_then(|t| path_of(self.root, t)) {
            node_at_mut(self.root, &path).anim.focus.set_target(if visible { 1.0 } else { 0.0 });
            self.call(&path, &WidgetEvent::FocusChanged { focused: true, visible, by });
        }
    }

    /// Tells the focus group around `target` which of its items had focus last.
    fn remember(&mut self, target: Option<NodeId>) {
        let Some(target) = target else { return };
        let Some(path) = path_of(self.root, target) else { return };
        let group = chain(self.root, &path)[..path.len()].iter().rposition(|n| n.input.group.is_some());
        if let Some(k) = group {
            node_at_mut(self.root, &path[..k]).memory = Some(target);
        }
    }

    fn window_focus(&mut self, focused: bool) {
        self.core.window_focused = focused;
        self.core.paint_dirty = true;
        if let Some(path) = self.core.focus.and_then(|f| path_of(self.root, f)) {
            self.call(&path, &WidgetEvent::WindowFocus(focused));
        }
    }

    fn key(&mut self, event: &Event, key: &KeyEvent) {
        self.core.mods = key.mods;
        if key.pressed {
            self.core.keyboard = true;
        }
        let focus_path = self.core.focus.and_then(|f| path_of(self.root, f));
        let path = focus_path.unwrap_or_default();
        if self.disabled_on(&path) {
            return;
        }
        // From the focused element up to the root: each widget, then each `on_key` handler.
        for k in (0..=path.len()).rev() {
            let at = &path[..k];
            if self.call(at, &WidgetEvent::Input(event)) {
                return;
            }
            if let Some(h) = &node_at(self.root, at).input.key
                && let Some(message) = h(key)
            {
                self.core.out.messages.push(message);
                return;
            }
        }
        if !key.pressed {
            return;
        }
        match key.key {
            vui_core::Key::Named(NamedKey::Tab) if !key.mods.cmd && !key.mods.ctrl && !key.mods.alt => {
                self.tab(key.mods.shift);
            }
            vui_core::Key::Named(NamedKey::Enter | NamedKey::Space)
                if !key.mods.any() && !key.repeat && (!path.is_empty() || self.core.focus.is_some()) =>
            {
                self.activate(&path);
            }
            vui_core::Key::Named(
                named @ (NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::Home | NamedKey::End),
            ) if !key.mods.any() => self.group_key(named),
            _ => {}
        }
    }

    /// Moves focus to the next (or previous) Tab stop, wrapping around. A focus group is one stop.
    fn tab(&mut self, backwards: bool) {
        let stops = collect_stops(self.root);
        let n = stops.len();
        if n == 0 {
            return;
        }
        let current = self.core.focus.and_then(|f| stops.iter().position(|s| s.members.contains(&f)));
        let next = match (current, backwards) {
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
            (None, false) => 0,
            (None, true) => n - 1,
        };
        self.focus_and_reveal(stops[next].entry, FocusBy::Tab);
    }

    /// Arrow keys, Home and End inside a focus group move focus between its items.
    fn group_key(&mut self, key: NamedKey) {
        let Some(focus) = self.core.focus else { return };
        let Some(path) = path_of(self.root, focus) else { return };
        let nodes = chain(self.root, &path);
        let Some(g) = nodes[..path.len()].iter().rposition(|n| n.input.group.is_some()) else { return };
        let Some(group) = nodes[g].input.group else { return };
        let items = group_items(nodes[g]);
        let Some(at) = items.iter().position(|&id| id == focus) else { return };
        let vertical = group.axis != Some(Axis::X);
        let horizontal = group.axis != Some(Axis::Y);
        let step: isize = match key {
            NamedKey::ArrowDown if vertical => 1,
            NamedKey::ArrowUp if vertical => -1,
            NamedKey::ArrowRight if horizontal => 1,
            NamedKey::ArrowLeft if horizontal => -1,
            NamedKey::Home => -(at as isize),
            NamedKey::End => items.len() as isize - 1 - at as isize,
            _ => return,
        };
        let last = items.len() as isize - 1;
        let to = at as isize + step;
        let to = match to {
            t if (0..=last).contains(&t) => t,
            t if group.wrap => t.rem_euclid(last + 1),
            _ => return,
        };
        if to as usize != at {
            self.focus_and_reveal(items[to as usize], FocusBy::Arrow);
        }
    }

    /// Focuses by keyboard and scrolls the element into view.
    fn focus_and_reveal(&mut self, id: NodeId, by: FocusBy) {
        self.set_focus_by(Some(id), true, by);
        if let Some(path) = path_of(self.root, id) {
            let rect = node_at(self.root, &path).rect;
            self.reveal(&path, rect);
        }
    }

    /// What Enter or Space on a focused element does: the widget gets to answer, else its click
    /// handler runs.
    pub fn activate(&mut self, path: &[usize]) {
        if self.call(path, &WidgetEvent::Action(&crate::access::AccessAction::Click)) {
            return;
        }
        let node = node_at(self.root, path);
        let click = Click { pos: node.rect.center(), mods: self.core.mods, button: PointerButton::Primary, count: 1 };
        if let Some(h) = &node.input.press {
            let message = h(&click);
            self.core.out.messages.push(message);
        }
        if let Some(h) = &node_at(self.root, path).input.click {
            let message = h(&click);
            self.core.out.messages.push(message);
        }
    }

    fn ime(&mut self, event: &Event, _ime: &ImeEvent) {
        if let Some(path) = self.core.focus.and_then(|f| path_of(self.root, f)) {
            self.call(&path, &WidgetEvent::Input(event));
        }
    }

    // ---- dropping files -----------------------------------------------------------------------

    fn drop_target_at(&self, pos: Point) -> Option<NodeId> {
        let path = hit(self.root, pos)?;
        let nodes = chain(self.root, &path);
        if nodes.iter().any(|n| n.input.disabled) {
            return None;
        }
        nodes.iter().rev().find(|n| n.input.drop.is_some()).map(|n| n.id)
    }

    fn set_drop_target(&mut self, target: Option<NodeId>) {
        if self.core.drop_target == target {
            return;
        }
        for (id, on) in [(self.core.drop_target, false), (target, true)] {
            if let Some(path) = id.and_then(|i| path_of(self.root, i)) {
                node_at_mut(self.root, &path).anim.drop.set_target(if on { 1.0 } else { 0.0 });
            }
        }
        self.core.drop_target = target;
        self.core.animating = true;
    }

    fn dropped(&mut self, pos: Point, paths: &[PathBuf]) {
        let target = self.drop_target_at(pos);
        self.set_drop_target(None);
        let Some(path) = target.and_then(|t| path_of(self.root, t)) else { return };
        if let Some(h) = &node_at(self.root, &path).input.drop {
            let message = h(paths.to_vec());
            self.core.out.messages.push(message);
        }
    }
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
