//! Docking: panels arranged in a tree of splits and tab stacks, which the user rearranges by
//! dragging tabs.
//!
//! ```text
//! Split(row) ─┬─ Stack [Watchlist]
//!             ├─ Split(column) ─┬─ Stack [Chart · Chart 2]
//!             │                 └─ Stack [Positions · Orders]
//!             └─ Stack [AI]
//! ```
//!
//! [`DockTree`] is the arrangement as data, with the operations a layout needs: activate, close,
//! move a panel next to (or into) another, resize. [`Docking`] adds the drag in progress, the lock,
//! undo, the card menu open and the card filling the dock, and turns [`DockMsg`]s into changes.
//! [`dock`] draws it: each stack as a card with one quiet header over the active panel, splitters
//! between the children of a split.
//!
//! ```text
//! ╭ AI ⌄   Plan  History  Scores            ● ⋯ ╮   the title, the panel's own views, its tools,
//! │ …                                            │   and the card's menu: the other panels in the
//! ╰──────────────────────────────────────────────╯   card, the app's items, fill the dock, close
//! ```
//!
//! There are no tabs: a card with one panel shows its name, and one holding several makes its name a
//! menu of them. With [`Docking::titles`] off a card shows no name, only a faint grip to drag it by,
//! and [`Docking::used`] says which card was pressed last, for the app to name elsewhere. Double-clicking a header fills the dock with that card until it is asked back.
//! Dragging a header shows five drop zones over every stack (the centre joins its panels; an edge
//! splits it); letting go anywhere else tears the panel off, for the app to open in a window of its
//! own. A panel from outside the dock (a rail, say) can be dragged in the same way: it is inserted
//! where it lands, and let go nowhere it does nothing.
//!
//! Like every widget here it is controlled: the app keeps the [`Docking`], passes its messages to
//! [`Docking::update`], and draws what the panels hold. The panel type is the app's own: an id, an
//! enum, anything `Clone + PartialEq`.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, PointerButton, Shadow, Size, Tone, size};
use vui_icons::set as icon_set;
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{Align, Base, Builder, Dirty, Div, Element, Env, EventCx, Focus, IntoElement, Overflow, PaintCx, Parent, Radius, Styled, Widget, WidgetEvent, builder, column, row, small};

use super::splitter;
use crate::{icon_button, lift};

/// The smallest a pane may be dragged to, in points.
pub const MIN_PANE: f32 = 96.0;
/// A pane dropped on an edge starts this big.
const NEW_PANE: f32 = 320.0;
/// How far the pointer must move with a tab held before it is a drag, not a click.
const DRAG_AFTER: f32 = 5.0;
/// Layout changes kept for undo.
const UNDO_DEPTH: usize = 32;

/// Which way a split lays out its children.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    /// Side by side.
    Row,
    /// One above another.
    Column,
}

/// Where a dragged panel lands on a stack.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Among the stack's tabs.
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// One node of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Node<P> {
    /// Children side by side or stacked. `sizes` holds each child's size in points; one child takes
    /// what is left instead, so a window can grow without the layout knowing its size: the first whose
    /// size is 0 or missing, else the last.
    Split { axis: SplitAxis, sizes: Vec<f32>, children: Vec<Node<P>> },
    /// Tabs, one of them shown.
    Stack { tabs: Vec<P>, active: usize },
}

impl<P: Clone + PartialEq> Node<P> {
    pub fn stack(tabs: Vec<P>) -> Node<P> {
        Node::Stack { tabs, active: 0 }
    }

    /// Children side by side, `sizes` points wide; the one sized 0 (or the first without a size)
    /// takes the rest.
    pub fn row(children: Vec<Node<P>>, sizes: Vec<f32>) -> Node<P> {
        Node::Split { axis: SplitAxis::Row, sizes, children }
    }

    pub fn column(children: Vec<Node<P>>, sizes: Vec<f32>) -> Node<P> {
        Node::Split { axis: SplitAxis::Column, sizes, children }
    }

    fn panels<'a>(&'a self, out: &mut Vec<&'a P>) {
        match self {
            Node::Stack { tabs, .. } => out.extend(tabs.iter()),
            Node::Split { children, .. } => children.iter().for_each(|c| c.panels(out)),
        }
    }

    fn shown<'a>(&'a self, out: &mut Vec<&'a P>) {
        match self {
            Node::Stack { tabs, active } => out.extend(tabs.get(*active)),
            Node::Split { children, .. } => children.iter().for_each(|c| c.shown(out)),
        }
    }

    /// The path to the stack holding `p`.
    fn find(&self, p: &P, path: &mut Vec<usize>) -> bool {
        match self {
            Node::Stack { tabs, .. } => tabs.contains(p),
            Node::Split { children, .. } => children.iter().enumerate().any(|(i, c)| {
                path.push(i);
                let found = c.find(p, path);
                if !found {
                    path.pop();
                }
                found
            }),
        }
    }

    fn at_mut(&mut self, path: &[usize]) -> &mut Node<P> {
        match (path.split_first(), self) {
            (None, node) => node,
            (Some((i, rest)), Node::Split { children, .. }) => children[*i].at_mut(rest),
            (Some(_), node) => node,
        }
    }

    fn at(&self, path: &[usize]) -> &Node<P> {
        match (path.split_first(), self) {
            (None, node) => node,
            (Some((i, rest)), Node::Split { children, .. }) => children[*i].at(rest),
            (Some(_), node) => node,
        }
    }

    /// Drops empty stacks and splits with one child, and joins a split into its parent when they
    /// lie the same way. Every split comes out with a size for each child and one grower, sized 0.
    fn tidy(&mut self) {
        if let Node::Split { axis, sizes, children } = self {
            children.iter_mut().for_each(Node::tidy);
            let g = grower(sizes, children.len());
            // Each child with its size, None for the one that grows.
            let mut kept: Vec<(Node<P>, Option<f32>)> = Vec::new();
            for (i, child) in std::mem::take(children).into_iter().enumerate() {
                let size = (i != g).then(|| sizes.get(i).copied().filter(|s| *s > 0.0).unwrap_or(DEFAULT_PANE));
                match child {
                    Node::Stack { tabs, .. } if tabs.is_empty() => {}
                    Node::Split { axis: a, sizes: inner, children: grand } if a == *axis => {
                        // The inner split's sizes stand. Its grower grows here too, or, if this
                        // child had a size, takes what of it the others leave.
                        let ig = grower(&inner, grand.len());
                        let used: f32 = inner.iter().enumerate().filter(|(j, _)| *j != ig).map(|(_, s)| *s).sum();
                        for (j, c) in grand.into_iter().enumerate() {
                            let s = if j == ig { size.map(|s| (s - used).max(MIN_PANE)) } else { Some(inner.get(j).copied().unwrap_or(DEFAULT_PANE)) };
                            kept.push((c, s));
                        }
                    }
                    other => kept.push((other, size)),
                }
            }
            // Exactly one grows: the first left without a size, or the last.
            let first = kept.iter().position(|(_, s)| s.is_none()).unwrap_or(kept.len().saturating_sub(1));
            *sizes = kept.iter().enumerate().map(|(i, (_, s))| if i == first { 0.0 } else { s.unwrap_or(DEFAULT_PANE) }).collect();
            *children = kept.into_iter().map(|(c, _)| c).collect();
            if children.len() == 1 {
                *self = children.pop().expect("one child");
            } else if children.is_empty() {
                *self = Node::stack(Vec::new());
            }
        }
    }
}

/// A pane's size when it has none.
const DEFAULT_PANE: f32 = 240.0;

/// The child of a split that takes what is left: the first sized 0 or without a size, else the last.
fn grower(sizes: &[f32], n: usize) -> usize {
    (0..n).find(|&i| sizes.get(i).is_none_or(|s| *s <= 0.0)).unwrap_or(n.saturating_sub(1))
}

/// An arrangement of panels.
#[derive(Clone, Debug, PartialEq)]
pub struct DockTree<P> {
    pub root: Node<P>,
}

impl<P: Clone + PartialEq> DockTree<P> {
    pub fn new(root: Node<P>) -> DockTree<P> {
        let mut tree = DockTree { root };
        tree.root.tidy();
        tree
    }

    /// Every panel in it, in order.
    pub fn panels(&self) -> Vec<&P> {
        let mut out = Vec::new();
        self.root.panels(&mut out);
        out
    }

    /// The panels on screen: each stack's active tab.
    pub fn shown(&self) -> Vec<&P> {
        let mut out = Vec::new();
        self.root.shown(&mut out);
        out
    }

    pub fn contains(&self, p: &P) -> bool {
        self.root.find(p, &mut Vec::new())
    }

    fn path_of(&self, p: &P) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        self.root.find(p, &mut path).then_some(path)
    }

    /// Shows `p` in its stack.
    pub fn activate(&mut self, p: &P) -> bool {
        let Some(path) = self.path_of(p) else { return false };
        if let Node::Stack { tabs, active } = self.root.at_mut(&path) {
            *active = tabs.iter().position(|t| t == p).unwrap_or(0);
        }
        true
    }

    /// Takes `p` out; an emptied stack goes, and the splits around it close up.
    pub fn close(&mut self, p: &P) -> bool {
        let Some(path) = self.path_of(p) else { return false };
        if let Node::Stack { tabs, active } = self.root.at_mut(&path) {
            let at = tabs.iter().position(|t| t == p).unwrap_or(0);
            tabs.remove(at);
            if *active >= tabs.len() || *active > at {
                *active = active.saturating_sub(1);
            }
        }
        self.root.tidy();
        true
    }

    /// Puts `p` (not in the tree yet) beside or among the tabs of the stack holding `target`. A
    /// missing target puts it beside everything, at the right.
    pub fn insert(&mut self, p: P, target: &P, zone: Zone) {
        let Some(path) = self.path_of(target) else {
            let root = std::mem::replace(&mut self.root, Node::stack(Vec::new()));
            self.root = Node::row(vec![root, Node::stack(vec![p])], vec![0.0, NEW_PANE]);
            self.root.tidy();
            return;
        };
        let node = self.root.at_mut(&path);
        match zone {
            Zone::Center => {
                if let Node::Stack { tabs, active } = node {
                    tabs.push(p);
                    *active = tabs.len() - 1;
                }
            }
            edge => {
                let axis = if matches!(edge, Zone::Left | Zone::Right) { SplitAxis::Row } else { SplitAxis::Column };
                let old = std::mem::replace(node, Node::stack(Vec::new()));
                let new = Node::stack(vec![p]);
                // The new pane starts at a third of a typical window, and the old one grows.
                let (children, sizes) = if matches!(edge, Zone::Left | Zone::Top) { (vec![new, old], vec![NEW_PANE, 0.0]) } else { (vec![old, new], vec![0.0, NEW_PANE]) };
                *node = Node::Split { axis, sizes, children };
            }
        }
        self.root.tidy();
    }

    /// Moves `p` beside or among the tabs of `target`'s stack. Nothing happens if either is missing
    /// or `p` is the only tab of the stack it would split.
    pub fn move_panel(&mut self, p: &P, target: &P, zone: Zone) -> bool {
        if !self.contains(p) || !self.contains(target) || (p == target && zone == Zone::Center) {
            return false;
        }
        if p == target {
            // Splitting a panel off its own stack: only if it has company there.
            let path = self.path_of(p).expect("contains");
            if matches!(self.root.at(&path), Node::Stack { tabs, .. } if tabs.len() < 2) {
                return false;
            }
            let other = match self.root.at(&path) {
                Node::Stack { tabs, .. } => tabs.iter().find(|t| *t != p).cloned(),
                Node::Split { .. } => None,
            };
            let Some(other) = other else { return false };
            self.close(p);
            self.insert(p.clone(), &other, zone);
            return true;
        }
        self.close(p);
        self.insert(p.clone(), target, zone);
        true
    }

    /// Moves the edge after child `index` of the split at `path` by `by` points, keeping every pane
    /// at least [`MIN_PANE`].
    pub fn resize(&mut self, path: &[usize], index: usize, by: f32) {
        if let Node::Split { sizes, children, .. } = self.root.at_mut(path) {
            let n = children.len();
            let g = grower(sizes, n);
            sizes.resize(n, DEFAULT_PANE);
            sizes[g] = 0.0;
            // Before the grower the edge moves the pane before it; after, the pane after it.
            let (i, by) = if index < g { (index, by) } else { (index + 1, -by) };
            if let Some(size) = sizes.get_mut(i).filter(|_| i != g) {
                *size = (*size + by).max(MIN_PANE);
            }
        }
    }
}

/// A panel being dragged, and the drop zone under it.
#[derive(Clone, Debug, PartialEq)]
pub struct DockDrag<P> {
    pub panel: P,
    pub over: Option<(P, Zone)>,
}

/// What the dock's controls send.
#[derive(Clone, Debug, PartialEq)]
pub enum DockMsg<P> {
    Activate(P),
    Close(P),
    Resize {
        path: Vec<usize>,
        index: usize,
        by: f32,
    },
    /// A tab started moving.
    Drag(P),
    /// The pointer entered (true) or left a drop zone of the stack holding the first panel.
    Over(P, Zone, bool),
    /// The dragged tab was let go.
    Drop,
    /// The card menu of the stack holding this panel opened, or (`None`) closed.
    Menu(Option<P>),
    /// The card holding this panel fills the dock, or (`None`) the layout comes back.
    Zoom(Option<P>),
    /// A press landed somewhere in the card showing this panel: it is the one last used. Sent
    /// beside whatever the press does, and changes nothing else (an open menu stays open).
    Used(P),
}

/// What a change means for the app, beyond the tree: a panel to open in a window of its own, or
/// one closed.
#[derive(Clone, Debug, PartialEq)]
pub enum DockEvent<P> {
    TearOff(P),
    Closed(P),
    /// A panel from outside the dock was dropped into it.
    Inserted(P),
}

/// A dock as an app keeps it: the tree, the drag in progress, the lock and undo.
#[derive(Clone, Debug, PartialEq)]
pub struct Docking<P> {
    pub tree: DockTree<P>,
    pub drag: Option<DockDrag<P>>,
    /// Nothing moves or closes while locked (⌘⇧L in a terminal); tabs still switch.
    pub locked: bool,
    /// The card whose menu is open, by any panel in it.
    pub menu: Option<P>,
    /// The card filling the dock, by any panel in it.
    pub zoom: Option<P>,
    /// The panel showing in the card last pressed: what an app names elsewhere, such as in the
    /// notch, when its cards carry no titles.
    pub used: Option<P>,
    /// Whether each card's header names its panel. Without, a faint grip is what is dragged, and
    /// its tooltip and accessible name say the panel.
    pub titles: bool,
    undo: Vec<DockTree<P>>,
}

impl<P: Clone + PartialEq> Docking<P> {
    pub fn new(tree: DockTree<P>) -> Docking<P> {
        Docking { tree, drag: None, locked: false, menu: None, zoom: None, used: None, titles: true, undo: Vec::new() }
    }

    /// Keeps the tree as it is now as one undo step: before the app changes it itself.
    pub fn remember(&mut self) {
        self.undo.push(self.tree.clone());
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    /// Puts the layout back as it was before the last move, close or resize run.
    pub fn undo(&mut self) -> bool {
        self.undo.pop().map(|t| self.tree = t).is_some()
    }

    /// The panel whose card fills the dock, if it is still in it.
    pub fn zoomed(&self) -> Option<&P> {
        self.zoom.as_ref().filter(|p| self.tree.contains(p))
    }

    pub fn update(&mut self, msg: DockMsg<P>) -> Option<DockEvent<P>> {
        if !matches!(msg, DockMsg::Menu(_) | DockMsg::Over(..) | DockMsg::Used(_)) {
            self.menu = None;
        }
        match msg {
            DockMsg::Activate(p) => {
                self.tree.activate(&p);
            }
            DockMsg::Menu(p) => self.menu = p,
            DockMsg::Used(p) => self.used = Some(p),
            DockMsg::Zoom(p) => self.zoom = p.filter(|p| self.tree.contains(p)),
            DockMsg::Close(p) if !self.locked => {
                self.remember();
                self.tree.close(&p);
                if self.zoom.as_ref().is_some_and(|z| !self.tree.contains(z)) {
                    self.zoom = None;
                }
                return Some(DockEvent::Closed(p));
            }
            DockMsg::Resize { path, index, by } if !self.locked => {
                // One undo step per drag: a run of resizes of the same edge replaces its own step.
                self.remember();
                self.tree.resize(&path, index, by);
            }
            DockMsg::Drag(p) if !self.locked => self.drag = Some(DockDrag { panel: p, over: None }),
            DockMsg::Over(target, zone, on) => {
                if let Some(drag) = &mut self.drag {
                    if on {
                        drag.over = Some((target, zone));
                    } else if drag.over == Some((target, zone)) {
                        drag.over = None;
                    }
                }
            }
            DockMsg::Drop => {
                let drag = self.drag.take()?;
                if !self.tree.contains(&drag.panel) {
                    let (target, zone) = drag.over?;
                    self.remember();
                    self.tree.insert(drag.panel.clone(), &target, zone);
                    return Some(DockEvent::Inserted(drag.panel));
                }
                match drag.over {
                    Some((target, zone)) => {
                        let before = self.tree.clone();
                        if self.tree.move_panel(&drag.panel, &target, zone) {
                            self.undo.push(before);
                        }
                    }
                    None => return Some(DockEvent::TearOff(drag.panel)),
                }
            }
            _ => {}
        }
        None
    }
}

// ── The drag handle ──────────────────────────────────────────────────────────────────────────────

type Send<M> = Rc<dyn Fn() -> M>;

/// The widget behind [`drag_source`]: invisible, laid over what can be dragged.
pub struct DragSourceWidget<M> {
    on_click: Option<Send<M>>,
    on_double: Option<Send<M>>,
    on_drag: Option<Send<M>>,
    on_drop: Option<Send<M>>,
}

/// Two clicks this close together are a double click.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

#[derive(Default)]
pub struct DragSourceState {
    from: Option<vui_core::Point>,
    dragging: bool,
    last_click: Option<std::time::Instant>,
}

impl<M: 'static> Widget<M> for DragSourceWidget<M> {
    type State = DragSourceState;

    fn create_state(&self) -> DragSourceState {
        DragSourceState::default()
    }

    fn update(&self, _old: &Self, _state: &mut DragSourceState) -> Dirty {
        Dirty::None
    }

    fn measure(&self, _state: &mut DragSourceState, _env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        size(known.x.unwrap_or(0.0), known.y.unwrap_or(0.0))
    }

    fn paint(&self, _state: &DragSourceState, _cx: &mut PaintCx) {}

    fn event(&self, state: &mut DragSourceState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let fire = |cx: &mut EventCx<M>, f: &Option<Send<M>>| {
            if let Some(f) = f {
                cx.emit(f());
            }
        };
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                state.from = Some(*pos);
                state.dragging = false;
                cx.capture_pointer();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                if let Some(from) = state.from
                    && !state.dragging
                    && ((pos.x - from.x).powi(2) + (pos.y - from.y).powi(2)).sqrt() > DRAG_AFTER
                {
                    state.dragging = true;
                    cx.start_drag();
                    fire(cx, &self.on_drag);
                }
                state.from.is_some()
            }
            WidgetEvent::Input(Event::PointerUp { .. }) if state.from.is_some() => {
                let dragged = state.dragging;
                state.from = None;
                state.dragging = false;
                cx.release_pointer();
                if dragged {
                    state.last_click = None;
                    fire(cx, &self.on_drop);
                } else {
                    let now = std::time::Instant::now();
                    let double = state.last_click.is_some_and(|at| now.duration_since(at) < DOUBLE_CLICK) && self.on_double.is_some();
                    state.last_click = if double { None } else { Some(now) };
                    fire(cx, if double { &self.on_double } else { &self.on_click });
                }
                true
            }
            _ => false,
        }
    }

    fn focusable(&self) -> Focus {
        Focus::Never
    }
}

/// A handle that tells a click from a drag. Lay it over what it moves (it fills its parent):
/// `on_click` for a press let go where it started, `on_drag` once it moves, `on_drop` when a drag
/// is let go. While it drags, hover follows the pointer, so drop targets light up with `hover_bg`
/// and hear `on_hover`.
pub struct DragSource<M: 'static> {
    base: Base<M>,
    widget: DragSourceWidget<M>,
}

builder!(DragSource<M> => DragSourceWidget<M>);

impl<M: Clone + 'static> DragSource<M> {
    pub fn on_click(mut self, msg: M) -> Self {
        self.widget.on_click = Some(Rc::new(move || msg.clone()));
        self
    }

    /// Sent instead of a second click when two come quickly.
    pub fn on_double_click(mut self, msg: M) -> Self {
        self.widget.on_double = Some(Rc::new(move || msg.clone()));
        self
    }

    pub fn on_drag(mut self, msg: M) -> Self {
        self.widget.on_drag = Some(Rc::new(move || msg.clone()));
        self
    }

    pub fn on_drop(mut self, msg: M) -> Self {
        self.widget.on_drop = Some(Rc::new(move || msg.clone()));
        self
    }
}

pub fn drag_source<M: 'static>() -> DragSource<M> {
    let source = DragSource { base: Base::new(), widget: DragSourceWidget { on_click: None, on_double: None, on_drag: None, on_drop: None } };
    source.absolute().top(0.0).left(0.0).right(0.0).bottom(0.0).cursor(CursorIcon::Pointer)
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────────

/// The space between two cards, which is also the splitter that resizes them.
const GAP: f32 = 6.0;
/// How small a sized pane gets when the dock is too small for it.
const MIN_SIZED: f32 = 160.0;
/// How small the pane that takes the rest gets before the sized ones give way.
const MIN_GROWER: f32 = 240.0;

/// What the app draws for one panel: its title and its content. The shown panel's title in its
/// card's header is keyed `tab-{title}`, the card's menu button `menu-{title}`, and while a drag is
/// on, the zones over a stack `zone-{title}-{left|…|center}`, named for its shown panel.
pub struct DockPanel<M: 'static> {
    pub title: String,
    pub body: Element<M>,
    /// Controls at the right of the header while the panel is shown: a picker of its own.
    pub tools: Option<Element<M>>,
    /// The panel's own views, beside its title in the header while it is shown.
    pub views: Option<Element<M>>,
    /// The app's items in the card's menu while the panel is shown, above fill and close.
    pub menu: Vec<Element<M>>,
    /// The widest and tallest the panel's card is drawn, past which it looks stretched. The card
    /// holding it, and the split around that card, are kept to it; dragging an edge stops there.
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
}

impl<M: 'static> DockPanel<M> {
    pub fn new(title: impl Into<String>, body: impl IntoElement<M>) -> DockPanel<M> {
        DockPanel { title: title.into(), body: body.into_element(), tools: None, views: None, menu: Vec::new(), max_width: None, max_height: None }
    }

    /// The widest the panel's card gets.
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    /// The tallest the panel's card gets.
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn tools(mut self, tools: impl IntoElement<M>) -> Self {
        self.tools = Some(tools.into_element());
        self
    }

    pub fn views(mut self, views: impl IntoElement<M>) -> Self {
        self.views = Some(views.into_element());
        self
    }

    pub fn menu(mut self, items: Vec<Element<M>>) -> Self {
        self.menu = items;
        self
    }
}

/// The most a pane may be across and down; `None` is as much as it is given.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Limit {
    w: Option<f32>,
    h: Option<f32>,
}

impl Limit {
    fn along(self, axis: SplitAxis) -> Option<f32> {
        if axis == SplitAxis::Row { self.w } else { self.h }
    }

    /// The panes of a split put together. Along the split they add up, with the gaps, only when
    /// every one has a most (one without takes whatever is left). Across it each pane is as tall
    /// as the split, so the strictest of them holds.
    fn of_split(axis: SplitAxis, parts: &[Limit]) -> Limit {
        let along: Option<f32> = parts.iter().map(|l| l.along(axis)).sum::<Option<f32>>().map(|s| s + GAP * parts.len().saturating_sub(1) as f32);
        let across = parts.iter().filter_map(|l| if axis == SplitAxis::Row { l.h } else { l.w }).reduce(f32::min);
        if axis == SplitAxis::Row { Limit { w: along, h: across } } else { Limit { w: across, h: along } }
    }
}

/// Draws a dock. `panel` draws each panel on show (and names the hidden ones' tabs); `on` wraps
/// the dock's messages in the app's.
pub fn dock<P, M>(docking: &Docking<P>, panel: impl Fn(&P, bool) -> DockPanel<M>, on: impl Fn(DockMsg<P>) -> M + Clone + 'static) -> Div<M>
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    let mut path = Vec::new();
    // A card filling the dock is drawn alone, as the stack it is in.
    if let Some(z) = docking.zoomed() {
        let path = docking.tree.path_of(z).unwrap_or_default();
        if let Node::Stack { tabs, active } = docking.tree.root.at(&path) {
            return stack(tabs, *active, docking, &panel, &on).0.grow(1.0).min_width(0.0).min_height(0.0);
        }
    }
    node(&docking.tree.root, docking, &panel, &on, &mut path).0.grow(1.0).min_width(0.0).min_height(0.0)
}

fn node<P, M>(n: &Node<P>, d: &Docking<P>, panel: &impl Fn(&P, bool) -> DockPanel<M>, on: &(impl Fn(DockMsg<P>) -> M + Clone + 'static), path: &mut Vec<usize>) -> (Div<M>, Limit)
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    match n {
        Node::Stack { tabs, active } => stack(tabs, *active, d, panel, on),
        Node::Split { axis, sizes, children } => {
            let last = children.len().saturating_sub(1);
            let g = grower(sizes, children.len());
            let mut drawn: Vec<(Div<M>, Limit)> = Vec::new();
            for (i, child) in children.iter().enumerate() {
                path.push(i);
                drawn.push(node(child, d, panel, on, path));
                path.pop();
            }
            let limits: Vec<Limit> = drawn.iter().map(|(_, l)| *l).collect();
            // Each sized pane's size, kept to its most.
            let size_of = |i: usize| {
                let size = sizes.get(i).copied().filter(|s| *s > 0.0).unwrap_or(DEFAULT_PANE);
                limits[i].along(*axis).map_or(size, |m| size.min(m).max(MIN_PANE))
            };
            let mut parts: Vec<Element<M>> = Vec::new();
            for (i, (pane, limit)) in drawn.into_iter().enumerate() {
                let pane = pane.min_width(0.0).min_height(0.0);
                let size = size_of(i);
                // When the window is too small for every pane's size, the sized panes give way
                // first, in proportion, down to a floor; the growing pane keeps a usable minimum,
                // and stops at its most.
                let pane = match (i == g, axis) {
                    (false, SplitAxis::Row) => pane.width(size).grow(0.0).shrink(1.0).min_width(MIN_SIZED.min(size)),
                    (false, SplitAxis::Column) => pane.height(size).grow(0.0).shrink(1.0).min_height(MIN_SIZED.min(size)),
                    (true, SplitAxis::Row) => {
                        let pane = pane.grow(1.0).basis(0.0).min_width(MIN_GROWER);
                        if let Some(m) = limit.w { pane.max_width(m.max(MIN_GROWER)) } else { pane }
                    }
                    (true, SplitAxis::Column) => {
                        let pane = pane.grow(1.0).basis(0.0).min_height(MIN_GROWER);
                        if let Some(m) = limit.h { pane.max_height(m.max(MIN_GROWER)) } else { pane }
                    }
                };
                // Across the split, a pane with a smaller most than the split stays at its most.
                let pane = match (axis, limit.h, limit.w) {
                    (SplitAxis::Row, Some(m), _) => pane.max_height(m),
                    (SplitAxis::Column, _, Some(m)) => pane.max_width(m),
                    _ => pane,
                };
                parts.push(pane.into_element());
                if i < last {
                    let (on, at) = (on.clone(), path.clone());
                    // The edge moves the sized pane beside it (see `DockTree::resize`); it stops
                    // where that pane reaches its most, and a pane saved larger snaps back to it.
                    let (j, sign) = if i < g { (i, 1.0) } else { (i + 1, -1.0) };
                    let (now, most) = (sizes.get(j).copied().filter(|s| *s > 0.0).unwrap_or(DEFAULT_PANE), limits.get(j).and_then(|l| l.along(*axis)));
                    let handle = splitter().name("Resize").on_drag(move |by: f32| {
                        let by = match most {
                            Some(m) if j != g => sign * ((now.min(m) + sign * by).min(m) - now),
                            _ => by,
                        };
                        on(DockMsg::Resize { path: at.clone(), index: i, by })
                    });
                    // Narrower than a lone splitter: it is also the gap between two cards.
                    let handle = if *axis == SplitAxis::Column { handle.vertical().height(GAP) } else { handle.width(GAP) };
                    let handle = if d.locked { handle.hidden(true) } else { handle };
                    parts.push(handle.into_element());
                }
            }
            let base = if *axis == SplitAxis::Row { row() } else { column() };
            (base.grow(1.0).min_width(0.0).min_height(0.0).children(parts), Limit::of_split(*axis, &limits))
        }
    }
}

fn stack<P, M>(tabs: &[P], active: usize, d: &Docking<P>, panel: &impl Fn(&P, bool) -> DockPanel<M>, on: &(impl Fn(DockMsg<P>) -> M + Clone + 'static)) -> (Div<M>, Limit)
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    let mut body = None;
    let mut shown = None;
    // Every panel's title, for the menu of a card holding several.
    let mut titles = Vec::new();
    // The card's most: the largest of its panels', so switching between them doesn't move the
    // layout; none if any of them has none.
    let (mut w, mut h): (Option<Vec<f32>>, Option<Vec<f32>>) = (Some(Vec::new()), Some(Vec::new()));
    for (i, p) in tabs.iter().enumerate() {
        let drawn = panel(p, i == active);
        titles.push((p.clone(), drawn.title.clone(), i == active));
        w = w.zip(drawn.max_width).map(|(mut v, m)| {
            v.push(m);
            v
        });
        h = h.zip(drawn.max_height).map(|(mut v, m)| {
            v.push(m);
            v
        });
        if i == active {
            let DockPanel { title, body: content, tools, views, menu, .. } = drawn;
            body = Some(content);
            shown = Some((title, tools, views, menu));
        }
    }
    let most = |v: Option<Vec<f32>>| v.and_then(|v| v.into_iter().reduce(f32::max));
    let limit = Limit { w: most(w), h: most(h) };
    let (Some((shown_title, tools, views, own_items)), Some(current)) = (shown, tabs.get(active).cloned()) else { return (column(), limit) };
    let several = tabs.len() > 1;
    let menu_open = d.menu.as_ref().is_some_and(|m| tabs.contains(m));
    let zoomed = d.zoomed().is_some_and(|z| tabs.contains(z));

    // The title: what is dragged, a double click fills the dock, and in a card of several it opens
    // the menu of them.
    let handle = drag_source().on_double_click(on(DockMsg::Zoom((!zoomed).then(|| current.clone()))));
    let handle = if several { handle.on_click(on(DockMsg::Menu((!menu_open).then(|| current.clone())))) } else { handle };
    let handle = if d.locked { handle } else { handle.on_drag(on(DockMsg::Drag(current.clone()))).on_drop(on(DockMsg::Drop)) };
    let title = row()
        .key(format!("tab-{shown_title}"))
        .name(shown_title.clone())
        .height(26.0)
        .padding_x(if d.titles { 6.0 } else { 4.0 })
        .gap(3.0)
        .shrink(0.0)
        .align_items(Align::Center)
        .rounded(Radius::Control)
        .hover_bg(Tone::Hover);
    let chevron = several.then(|| crate::icon(icon_set::CHEVRON_DOWN).square(11.0).color(Tone::Text3));
    let title = if d.titles {
        title.children((small(shown_title.clone()).weight(600).color(Tone::Text).max_lines(1), chevron, handle))
    } else {
        // No name: a faint grip to drag the card by, which still names it to the pointer and to
        // assistive tools.
        title.tooltip(shown_title.clone()).children((crate::icon(icon_set::GRIP_VERTICAL).square(12.0).color(Tone::Text3), chevron, handle))
    };

    // The card's menu: the other panels, the app's items, then fill and close.
    let mut items: Vec<Element<M>> = Vec::new();
    if several {
        items.extend(titles.iter().map(|(p, t, on_now)| crate::menu_check(t.clone(), *on_now).key(format!("card-tab-{t}")).on_click(on(DockMsg::Activate(p.clone()))).into_element()));
        items.push(crate::menu_separator().into_element());
    }
    let has_own = !own_items.is_empty();
    items.extend(own_items);
    if has_own {
        items.push(crate::menu_separator().into_element());
    }
    let fill = if zoomed { "Back to the layout" } else { "Fill the desk" };
    items.push(crate::menu_item(fill).key(format!("card-zoom-{shown_title}")).on_click(on(DockMsg::Zoom((!zoomed).then(|| current.clone())))).into_element());
    if !d.locked {
        items.push(crate::menu_item("Close").key(format!("card-close-{shown_title}")).on_click(on(DockMsg::Close(current.clone()))).into_element());
    }
    let more = icon_button(icon_set::MORE, "More").key(format!("menu-{shown_title}")).ghost().small().on_click(on(DockMsg::Menu((!menu_open).then(|| current.clone()))));
    let more = crate::dropdown(more, menu_open, on(DockMsg::Menu(None)), items).side(crate::Side::BelowEnd);

    let bar = row().height(26.0).gap(6.0).padding_x(2.0).align_items(Align::Center).min_width(0.0).children((
        title,
        // The views take the room left, so they can ask how wide it is (`when_container`).
        match views {
            Some(v) => row().grow(1.0).basis(0.0).min_width(0.0).align_items(Align::Center).overflow(Overflow::Hidden).child(v),
            None => row().grow(1.0),
        },
        tools.map(|t| row().shrink(0.0).align_items(Align::Center).child(t)),
        more,
    ));
    let target = tabs.get(active).cloned();
    let zones = match (&d.drag, target) {
        (Some(_), Some(target)) => Some(drop_zones(target, &shown_title, d, on)),
        _ => None,
    };
    // Each stack is a card on the canvas, with no outline. The one in focus rises on a soft shadow and
    // a faint glow; the others sink half-way into the canvas colour, so panels read as separate
    // pieces by depth and the gaps between them are the splitters. The focus is the stack holding
    // the card pressed last, whichever of its tabs shows; before anything is pressed, the first on
    // show. A card pressed outside the dock (one the app draws beside it) leaves every stack at rest.
    let focused = zoomed
        || match &d.used {
            Some(u) => tabs.contains(u),
            None => d.tree.shown().first().copied() == Some(&current),
        };
    let card = column()
        .key(format!("card-{shown_title}"))
        .grow(1.0)
        .min_width(0.0)
        .min_height(0.0)
        .gap(0.0)
        .padding(CARD_PADDING)
        .rounded(Radius::Card)
        .overflow(Overflow::Hidden)
        .on_press_within(on(DockMsg::Used(current.clone())))
        .children((bar, column().grow(1.0).min_height(0.0).min_width(0.0).children(body), zones));
    (raised(&shown_title, focused, card), limit)
}

/// Space between a card's edge and what it holds.
const CARD_PADDING: f32 = 4.0;
/// How much of the sheet's colour a card keeps while it is not in focus: it fades into the canvas
/// around it rather than being outlined against it.
const FADED: f32 = 0.55;

/// A card that rises when `focused` and sinks into the canvas when not: its surface, the shadow it
/// casts and a faint accent glow all move together, with a spring (see [`lift`]). `name` keys the
/// lift, `card` is what sits on it. For an app's own cards that sit beside the dock's, so they all
/// look alike.
pub fn raised<M: 'static>(name: &str, focused: bool, card: Div<M>) -> Div<M> {
    let surface = lift(focused)
        .key(format!("lift-{name}"))
        .grow(1.0)
        .min_width(0.0)
        .min_height(0.0)
        .rounded(Radius::Card)
        .surface(Fill::Tone(Tone::Bg, FADED), Tone::Bg)
        .shadow(Shadow { blur: 30.0, y: 12.0, spread: -8.0, alpha: 0.2 })
        .glow(Fill::Tone(Tone::Accent, 0.5), -1.0, 16.0)
        .child(card.rounded(Radius::Card));
    column().grow(1.0).min_width(0.0).min_height(0.0).child(surface)
}

/// Five zones over a stack while a tab is dragged: each edge splits the stack, the centre joins it.
fn drop_zones<P, M>(target: P, title: &str, d: &Docking<P>, on: &(impl Fn(DockMsg<P>) -> M + Clone + 'static)) -> Div<M>
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    let lit = d.drag.as_ref().and_then(|g| g.over.clone());
    let zone = |z: Zone, weight: f32| {
        let on = on.clone();
        let t = target.clone();
        let here = lit.as_ref().is_some_and(|(p, lz)| *p == target && *lz == z);
        column()
            .key(format!("zone-{title}-{z:?}").to_lowercase())
            .grow(weight)
            .basis(0.0)
            .min_width(0.0)
            .min_height(0.0)
            .rounded(Radius::Control)
            .bg(if here { Fill::Tone(Tone::Accent, 0.22) } else { Fill::NONE })
            .on_hover(move |inside| on(DockMsg::Over(t.clone(), z, inside)))
    };
    row().absolute().top(0.0).left(0.0).right(0.0).bottom(0.0).rounded(Radius::Card).border(1.0, Fill::Tone(Tone::Accent, 0.35)).children((
        zone(Zone::Left, 1.0),
        column().grow(2.0).basis(0.0).min_width(0.0).children((zone(Zone::Top, 1.0), zone(Zone::Center, 2.0), zone(Zone::Bottom, 1.0))),
        zone(Zone::Right, 1.0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> DockTree<&'static str> {
        DockTree::new(Node::row(
            vec![Node::stack(vec!["watch"]), Node::column(vec![Node::stack(vec!["chart", "chart2"]), Node::stack(vec!["book"])], vec![400.0]), Node::stack(vec!["ai"])],
            vec![260.0, 700.0],
        ))
    }

    #[test]
    fn closing_the_last_tab_of_a_stack_closes_the_split_around_it() {
        let mut t = layout();
        assert!(t.close(&"book"));
        assert_eq!(t.root, Node::row(vec![Node::stack(vec!["watch"]), Node::stack(vec!["chart", "chart2"]), Node::stack(vec!["ai"])], vec![260.0, 700.0, 0.0]));
        assert_eq!(t.shown(), vec![&"watch", &"chart", &"ai"]);
    }

    #[test]
    fn a_panel_dropped_on_an_edge_splits_that_stack_and_on_the_centre_joins_it() {
        let mut t = layout();
        assert!(t.move_panel(&"ai", &"book", Zone::Right));
        let Node::Split { children, .. } = &t.root else { panic!() };
        assert_eq!(children.len(), 2, "the AI's stack went, so the row has two children");
        assert_eq!(t.shown(), vec![&"watch", &"chart", &"book", &"ai"]);
        assert!(t.move_panel(&"ai", &"chart", Zone::Center));
        assert_eq!(t.shown(), vec![&"watch", &"ai", &"book"], "it joined the chart's tabs, shown");
        assert_eq!(t.panels().len(), 5);
    }

    #[test]
    fn a_tab_split_off_its_own_stack_needs_company_there() {
        let mut t = layout();
        assert!(!t.move_panel(&"book", &"book", Zone::Bottom), "book is alone in its stack");
        assert!(t.move_panel(&"chart2", &"chart2", Zone::Bottom));
        assert_eq!(t.shown(), vec![&"watch", &"chart", &"chart2", &"book", &"ai"]);
    }

    #[test]
    fn resizing_keeps_a_minimum_and_undo_puts_it_back() {
        let mut d = Docking::new(layout());
        d.update(DockMsg::Resize { path: vec![], index: 0, by: -500.0 });
        let Node::Split { sizes, .. } = &d.tree.root else { panic!() };
        assert_eq!(sizes[0], MIN_PANE);
        assert!(d.undo());
        assert_eq!(d.tree, layout());
    }

    #[test]
    fn the_middle_pane_can_grow_and_the_edge_after_it_moves_the_one_after() {
        let mut t = DockTree::new(Node::row(vec![Node::stack(vec!["watch"]), Node::stack(vec!["chart"]), Node::stack(vec!["ai"])], vec![248.0, 0.0, 360.0]));
        t.resize(&[], 1, -40.0);
        let Node::Split { sizes, .. } = &t.root else { panic!() };
        assert_eq!(sizes, &vec![248.0, 0.0, 400.0], "dragging the AI's edge left widens it");
        assert!(t.close(&"watch"));
        let Node::Split { sizes, .. } = &t.root else { panic!() };
        assert_eq!(sizes, &vec![0.0, 400.0], "the chart still grows");
    }

    #[test]
    fn a_drag_lands_where_the_pointer_last_was_or_tears_off() {
        let mut d = Docking::new(layout());
        d.update(DockMsg::Drag("chart2"));
        d.update(DockMsg::Over("ai", Zone::Left, true));
        d.update(DockMsg::Over("ai", Zone::Left, false));
        d.update(DockMsg::Over("ai", Zone::Bottom, true));
        assert_eq!(d.update(DockMsg::Drop), None);
        assert_eq!(d.tree.shown(), vec![&"watch", &"chart", &"book", &"ai", &"chart2"]);
        d.update(DockMsg::Drag("watch"));
        assert_eq!(d.update(DockMsg::Drop), Some(DockEvent::TearOff("watch")), "let go over no zone");
        d.locked = true;
        d.update(DockMsg::Close("ai"));
        assert!(d.tree.contains(&"ai"), "nothing closes while locked");
    }

    #[test]
    fn a_panel_from_outside_lands_where_dropped_and_nowhere_does_nothing() {
        let mut d = Docking::new(layout());
        d.update(DockMsg::Drag("news"));
        assert_eq!(d.update(DockMsg::Drop), None, "let go over no zone: not torn off, not added");
        assert!(!d.tree.contains(&"news"));
        d.update(DockMsg::Drag("news"));
        d.update(DockMsg::Over("book", Zone::Right, true));
        assert_eq!(d.update(DockMsg::Drop), Some(DockEvent::Inserted("news")));
        assert_eq!(d.tree.shown(), vec![&"watch", &"chart", &"book", &"news", &"ai"]);
        assert!(d.undo() && !d.tree.contains(&"news"));
    }

    #[test]
    fn a_card_fills_the_dock_until_asked_back_and_its_menu_closes_on_any_change() {
        let mut d = Docking::new(layout());
        d.update(DockMsg::Menu(Some("chart2")));
        assert_eq!(d.menu, Some("chart2"));
        d.update(DockMsg::Zoom(Some("chart2")));
        assert_eq!((d.zoomed(), d.menu), (Some(&"chart2"), None));
        d.update(DockMsg::Zoom(Some("missing")));
        assert_eq!(d.zoomed(), None, "only a panel in the dock fills it");
        d.update(DockMsg::Zoom(Some("book")));
        d.update(DockMsg::Close("book"));
        assert_eq!(d.zoomed(), None, "closing the card brings the layout back");
    }
}
