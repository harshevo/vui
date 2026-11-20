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
//! move a panel next to (or into) another, resize. [`Docking`] adds the drag in progress, the lock
//! and undo, and turns [`DockMsg`]s into changes. [`dock`] draws it: each stack's tabs over the
//! active panel, splitters between the children of a split. Dragging a tab shows five drop zones
//! over every stack (the centre joins its tabs; an edge splits it); letting go anywhere else tears
//! the panel off, for the app to open in a window of its own.
//!
//! Like every widget here it is controlled: the app keeps the [`Docking`], passes its messages to
//! [`Docking::update`], and draws what the panels hold. The panel type is the app's own: an id, an
//! enum, anything `Clone + PartialEq`.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, PointerButton, Size, Tone, size};
use vui_icons::set as icon_set;
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    Align, Base, Builder, Dirty, Div, Element, Env, EventCx, Focus, IntoElement, Overflow, PaintCx, Parent, Radius, Styled, Widget, WidgetEvent,
    builder, column, row, small,
};

use crate::icon_button;
use super::splitter;

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

/// A tab being dragged, and the drop zone under it.
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
    Resize { path: Vec<usize>, index: usize, by: f32 },
    /// A tab started moving.
    Drag(P),
    /// The pointer entered (true) or left a drop zone of the stack holding the first panel.
    Over(P, Zone, bool),
    /// The dragged tab was let go.
    Drop,
}

/// What a change means for the app, beyond the tree: a panel to open in a window of its own, or
/// one closed.
#[derive(Clone, Debug, PartialEq)]
pub enum DockEvent<P> {
    TearOff(P),
    Closed(P),
}

/// A dock as an app keeps it: the tree, the drag in progress, the lock and undo.
#[derive(Clone, Debug, PartialEq)]
pub struct Docking<P> {
    pub tree: DockTree<P>,
    pub drag: Option<DockDrag<P>>,
    /// Nothing moves or closes while locked (⌘⇧L in a terminal); tabs still switch.
    pub locked: bool,
    undo: Vec<DockTree<P>>,
}

impl<P: Clone + PartialEq> Docking<P> {
    pub fn new(tree: DockTree<P>) -> Docking<P> {
        Docking { tree, drag: None, locked: false, undo: Vec::new() }
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

    pub fn update(&mut self, msg: DockMsg<P>) -> Option<DockEvent<P>> {
        match msg {
            DockMsg::Activate(p) => {
                self.tree.activate(&p);
            }
            DockMsg::Close(p) if !self.locked => {
                self.remember();
                self.tree.close(&p);
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
    on_drag: Option<Send<M>>,
    on_drop: Option<Send<M>>,
}

#[derive(Default)]
pub struct DragSourceState {
    from: Option<vui_core::Point>,
    dragging: bool,
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
                *state = DragSourceState { from: Some(*pos), dragging: false };
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
                *state = DragSourceState::default();
                cx.release_pointer();
                fire(cx, if dragged { &self.on_drop } else { &self.on_click });
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
    let source = DragSource { base: Base::new(), widget: DragSourceWidget { on_click: None, on_drag: None, on_drop: None } };
    source.absolute().top(0.0).left(0.0).right(0.0).bottom(0.0).cursor(CursorIcon::Pointer)
}

// ── Drawing ──────────────────────────────────────────────────────────────────────────────────────

/// The space between two cards, which is also the splitter that resizes them.
const GAP: f32 = 6.0;

/// What the app draws for one panel: its tab's title and its content. The tab is keyed
/// `tab-{title}`, and while a drag is on, the zones over a stack `zone-{title}-{left|…|center}`,
/// named for its shown tab.
pub struct DockPanel<M: 'static> {
    pub title: String,
    pub body: Element<M>,
    /// Controls at the right of the tab bar while the panel is shown: a picker of its own.
    pub tools: Option<Element<M>>,
}

impl<M: 'static> DockPanel<M> {
    pub fn new(title: impl Into<String>, body: impl IntoElement<M>) -> DockPanel<M> {
        DockPanel { title: title.into(), body: body.into_element(), tools: None }
    }

    pub fn tools(mut self, tools: impl IntoElement<M>) -> Self {
        self.tools = Some(tools.into_element());
        self
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
    node(&docking.tree.root, docking, &panel, &on, &mut path).grow(1.0).min_width(0.0).min_height(0.0)
}

fn node<P, M>(n: &Node<P>, d: &Docking<P>, panel: &impl Fn(&P, bool) -> DockPanel<M>, on: &(impl Fn(DockMsg<P>) -> M + Clone + 'static), path: &mut Vec<usize>) -> Div<M>
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    match n {
        Node::Stack { tabs, active } => stack(tabs, *active, d, panel, on),
        Node::Split { axis, sizes, children } => {
            let mut parts: Vec<Element<M>> = Vec::new();
            let last = children.len().saturating_sub(1);
            let g = grower(sizes, children.len());
            for (i, child) in children.iter().enumerate() {
                path.push(i);
                let drawn = node(child, d, panel, on, path).min_width(0.0).min_height(0.0);
                path.pop();
                let size = sizes.get(i).copied().filter(|s| *s > 0.0).unwrap_or(DEFAULT_PANE);
                let drawn = match (i == g, axis) {
                    (false, SplitAxis::Row) => drawn.width(size).grow(0.0).shrink(0.0),
                    (false, SplitAxis::Column) => drawn.height(size).grow(0.0).shrink(0.0),
                    (true, _) => drawn.grow(1.0).basis(0.0),
                };
                parts.push(drawn.into_element());
                if i < last {
                    let (on, at) = (on.clone(), path.clone());
                    let handle = splitter().name("Resize").on_drag(move |by| on(DockMsg::Resize { path: at.clone(), index: i, by }));
                    // Narrower than a lone splitter: it is also the gap between two cards.
                    let handle = if *axis == SplitAxis::Column { handle.vertical().height(GAP) } else { handle.width(GAP) };
                    let handle = if d.locked { handle.hidden(true) } else { handle };
                    parts.push(handle.into_element());
                }
            }
            let base = if *axis == SplitAxis::Row { row() } else { column() };
            base.grow(1.0).min_width(0.0).min_height(0.0).children(parts)
        }
    }
}

fn stack<P, M>(tabs: &[P], active: usize, d: &Docking<P>, panel: &impl Fn(&P, bool) -> DockPanel<M>, on: &(impl Fn(DockMsg<P>) -> M + Clone + 'static)) -> Div<M>
where
    P: Clone + PartialEq + 'static,
    M: Clone + 'static,
{
    let mut body = None;
    let mut tools = None;
    let mut shown_title = String::new();
    let heads: Vec<Div<M>> = tabs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let shown = i == active;
            let DockPanel { title, body: content, tools: own } = panel(p, shown);
            if shown {
                body = Some(content);
                tools = own;
                shown_title = title.clone();
            }
            let handle = drag_source().on_click(on(DockMsg::Activate(p.clone())));
            let handle = if d.locked { handle } else { handle.on_drag(on(DockMsg::Drag(p.clone()))).on_drop(on(DockMsg::Drop)) };
            let close = (!d.locked && shown).then(|| icon_button(icon_set::X, "Close").on_click(on(DockMsg::Close(p.clone()))).square(18.0));
            row()
                .key(format!("tab-{title}"))
                .name(title.clone())
                .height(26.0)
                .padding_x(10.0)
                .gap(4.0)
                .align_items(Align::Center)
                .rounded(Radius::Pill)
                .bg(if shown { Fill::from(Tone::Wash) } else { Fill::NONE })
                .hover_bg(if shown { Tone::Wash } else { Tone::Hover })
                .children((small(title).weight(500).color(if shown { Tone::Text } else { Tone::Text2 }).max_lines(1), handle, close))
        })
        .collect();
    let bar = row().height(30.0).gap(2.0).align_items(Align::Center).min_width(0.0).children((
        row().grow(1.0).gap(2.0).align_items(Align::Center).min_width(0.0).overflow(Overflow::Hidden).children(heads),
        tools.map(|t| row().shrink(0.0).align_items(Align::Center).child(t)),
    ));
    let target = tabs.get(active).cloned();
    let zones = match (&d.drag, target) {
        (Some(_), Some(target)) => Some(drop_zones(target, &shown_title, d, on)),
        _ => None,
    };
    // Each stack is a card: the theme's continuous card corners, a hairline edge and the faintest
    // lift off the sheet, so panels read as separate pieces while the splitters between them are
    // the gaps.
    column()
        .key(format!("card-{shown_title}"))
        .grow(1.0)
        .min_width(0.0)
        .min_height(0.0)
        .gap(4.0)
        .padding(4.0)
        .rounded(Radius::Card)
        .bg(Fill::Tone(Tone::Text, 0.02))
        .border(1.0, Fill::Tone(Tone::Text, 0.08))
        .overflow(Overflow::Hidden)
        .children((bar, column().grow(1.0).min_height(0.0).min_width(0.0).children(body), zones))
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
    row()
        .absolute()
        .top(0.0)
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .rounded(Radius::Card)
        .border(1.0, Fill::Tone(Tone::Accent, 0.35))
        .children((
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
}
