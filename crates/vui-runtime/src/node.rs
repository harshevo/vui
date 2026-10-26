//! The retained tree: one [`Node`] per element, kept between updates.
//!
//! The app builds a fresh [`Element`] tree after every change. Reconciling it against the nodes the
//! runtime already has keeps each node's widget state, springs, scroll position and layout cache, and
//! reports how much work the change needs: nothing, a repaint, or a new layout.

use std::any::Any;
use std::collections::HashMap;

use vui_core::{Color, Insets, Palette, Point, Rect, Spring, SpringValue, Vec2, pt, rect, vec2};

use crate::access::A11y;
use crate::element::{Base, Decor, Element, Input};
use crate::key::{Key, NodeId};
use crate::layout::{Avail, LayoutCache, LayoutNode, NodeLayout, Pair};
use crate::style::{Axis, Len, Overflow, Style};
use crate::widget::{Dirty, Env, Erased, Focus, TickCx};

/// Hover, press, focus and drop fades: critically damped, so they never overshoot a 0 to 1 amount.
pub(crate) const FAST: Spring = Spring::new(2000.0, 89.44, 1.0);
/// Wheel scrolling.
const SCROLL: Spring = Spring::new(500.0, 45.0, 1.0);
/// Scrollbars fading in and out, and growing under the pointer.
const BARS: Spring = Spring::new(400.0, 40.0, 1.0);

pub(crate) const BAR_WIDTH: f32 = 6.0;
pub(crate) const BAR_WIDTH_HOT: f32 = 9.0;
const BAR_INSET: f32 = 2.0;
const BAR_MIN: f32 = 28.0;
/// Seconds the scrollbars stay up after the last scroll.
const BAR_HOLD: f32 = 0.9;

/// Hands out node ids.
#[derive(Default)]
pub(crate) struct Ids(u64);

impl Ids {
    pub fn next(&mut self) -> NodeId {
        self.0 += 1;
        NodeId(self.0)
    }
}

struct FillAnim {
    from: Color,
    t: SpringValue,
}

/// Everything about a node that is moving.
pub(crate) struct Anim {
    pub hover: SpringValue,
    pub press: SpringValue,
    pub focus: SpringValue,
    pub drop: SpringValue,
    pub opacity: SpringValue,
    width: Option<SpringValue>,
    height: Option<SpringValue>,
    fill: Option<FillAnim>,
}

impl Anim {
    fn new(opacity: f32) -> Anim {
        Anim {
            hover: SpringValue::new(0.0, FAST),
            press: SpringValue::new(0.0, FAST),
            focus: SpringValue::new(0.0, FAST),
            drop: SpringValue::new(0.0, FAST),
            opacity: SpringValue::new(opacity, FAST),
            width: None,
            height: None,
            fill: None,
        }
    }
}

/// A scroll container's position and scrollbars.
pub(crate) struct Scroll {
    pub x: SpringValue,
    pub y: SpringValue,
    /// How far each axis can scroll, as of the last arrange.
    pub max: Vec2,
    /// 0 to 1: how visible the scrollbars are.
    pub bars: SpringValue,
    /// 0 to 1: how fat the bar under the pointer has grown.
    pub thick: SpringValue,
    /// Seconds since the scrollbars were last wanted.
    pub idle: f32,
}

impl Scroll {
    fn new() -> Scroll {
        Scroll {
            x: SpringValue::new(0.0, SCROLL),
            y: SpringValue::new(0.0, SCROLL),
            max: Vec2::ZERO,
            bars: SpringValue::new(0.0, BARS),
            thick: SpringValue::new(0.0, BARS),
            idle: BAR_HOLD,
        }
    }

    /// Where the content is now.
    pub fn offset(&self) -> Vec2 {
        vec2(self.x.value().clamp(0.0, self.max.x), self.y.value().clamp(0.0, self.max.y))
    }

    /// Where the content is heading.
    pub fn target(&self) -> Vec2 {
        vec2(self.x.target().clamp(0.0, self.max.x), self.y.target().clamp(0.0, self.max.y))
    }

    pub fn max_along(&self, axis: Axis) -> f32 {
        match axis {
            Axis::X => self.max.x,
            Axis::Y => self.max.y,
        }
    }

    pub fn spring_mut(&mut self, axis: Axis) -> &mut SpringValue {
        match axis {
            Axis::X => &mut self.x,
            Axis::Y => &mut self.y,
        }
    }

    /// Shows the scrollbars for a moment.
    pub fn poke(&mut self) {
        self.idle = 0.0;
    }

    /// Records how far the content can scroll, pulling the position back if the content shrank.
    fn set_max(&mut self, max: Vec2) {
        self.max = max;
        if self.x.value() > max.x || self.x.target() > max.x {
            self.x.snap_to(self.x.value().min(max.x));
        }
        if self.y.value() > max.y || self.y.target() > max.y {
            self.y.snap_to(self.y.value().min(max.y));
        }
    }
}

/// A scrollbar thumb and how far it can travel.
pub(crate) struct Bar {
    pub thumb: Rect,
    /// Pixels the thumb can move along its track.
    pub free: f32,
    /// Pixels the content can scroll.
    pub max: f32,
}

pub(crate) struct Node<M: 'static> {
    pub id: NodeId,
    pub key: Option<Key>,
    pub widget: Box<dyn Erased<M>>,
    pub state: Box<dyn Any>,
    /// What the element asked for. `eff` follows it, one step behind while a size is animating.
    pub target: Style,
    /// What layout reads: `style`, with sizes that are mid-transition replaced by their current value.
    pub eff: Style,
    pub decor: Decor,
    pub input: Input<M>,
    pub a11y: A11y,
    pub transition: Option<Spring>,
    pub children: Vec<Node<M>>,
    pub cache: LayoutCache,
    pub layout: NodeLayout,
    /// Border box in window coordinates, as of the last arrange.
    pub rect: Rect,
    /// What ancestors cut away, in window coordinates.
    pub clip: Option<Rect>,
    pub scroll: Scroll,
    pub anim: Anim,
}

impl<M: 'static> LayoutNode<Env> for Node<M> {
    fn style(&self) -> &Style {
        &self.eff
    }

    fn children_mut(&mut self) -> &mut [Node<M>] {
        &mut self.children
    }

    fn measure(&mut self, cx: &mut Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> vui_core::Size {
        self.widget.measure(&mut *self.state, cx, known, avail)
    }

    fn baseline(&self) -> Option<f32> {
        self.widget.baseline(&*self.state)
    }

    fn cache(&mut self) -> &mut LayoutCache {
        &mut self.cache
    }

    fn layout_mut(&mut self) -> &mut NodeLayout {
        &mut self.layout
    }
}

/// What a tick found out, gathered over the whole tree.
pub(crate) struct TickState<'a> {
    pub env: &'a Env,
    pub time: f32,
    pub window_focused: bool,
    pub focus: Option<NodeId>,
    /// Soonest time anything asked to be woken, in seconds from now.
    pub wake: Option<f32>,
    /// Something is still moving: tick again next frame.
    pub moving: bool,
    pub layout: bool,
    pub arrange: bool,
    pub paint: bool,
}

/// Steps a spring. Returns true if its value changed.
fn advance(spring: &mut SpringValue, dt: f32, ts: &mut TickState) -> bool {
    let before = spring.value();
    if spring.step(dt) {
        ts.moving = true;
    }
    let changed = spring.value() != before;
    if changed {
        ts.paint = true;
    }
    changed
}

/// Blends two colours without the dark fringe that mixing straight-alpha colours gives next to
/// transparent ones.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let (pa, pb) = (a.premultiplied(), b.premultiplied());
    let m = |i: usize| pa[i] + (pb[i] - pa[i]) * t;
    let alpha = m(3);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    Color::rgba(m(0) / alpha, m(1) / alpha, m(2) / alpha, alpha)
}

/// Starts, steers or ends the animation of one size, given the old and new lengths.
fn retarget(current: Option<SpringValue>, old: Len, new: Len, spring: Option<Spring>) -> Option<SpringValue> {
    let (Some(spring), Len::Px(to)) = (spring, new) else {
        return None;
    };
    match (current, old) {
        (Some(mut a), _) => {
            a.spring = spring;
            a.set_target(to);
            Some(a)
        }
        (None, Len::Px(from)) if from != to => {
            let mut a = SpringValue::new(from, spring);
            a.set_target(to);
            Some(a)
        }
        _ => None,
    }
}

impl<M: 'static> Node<M> {
    pub fn build(el: Element<M>, ids: &mut Ids) -> Node<M> {
        let Element { base, widget, children } = el;
        let Base { style, decor, input, a11y, key, transition } = base;
        let opacity = decor.opacity;
        let state = widget.create_state();
        Node {
            id: ids.next(),
            key,
            widget,
            state,
            target: style,
            eff: style,
            decor,
            input,
            a11y,
            transition,
            children: children.into_iter().map(|c| Node::build(c, ids)).collect(),
            cache: LayoutCache::default(),
            layout: NodeLayout::default(),
            rect: Rect::ZERO,
            clip: None,
            scroll: Scroll::new(),
            anim: Anim::new(opacity),
        }
    }

    /// Brings the node in line with a new element for the same place in the tree and says how much
    /// has to be redone.
    pub fn update(&mut self, el: Element<M>, ids: &mut Ids, env: &Env) -> Dirty {
        let Element { base, widget, children } = el;
        let Base { style, decor, input, a11y, key: _, transition } = base;

        let mut dirty = widget.update(&*self.widget, &mut *self.state);
        self.widget = widget;
        self.transition = transition;

        if style != self.target {
            self.anim.width = retarget(self.anim.width.take(), self.target.width, style.width, transition);
            self.anim.height = retarget(self.anim.height.take(), self.target.height, style.height, transition);
            self.target = style;
        }
        let before = self.eff;
        self.refresh_eff();
        if self.eff != before {
            dirty = dirty.max(Dirty::Layout);
        }

        if decor != self.decor {
            self.retarget_decor(&decor, env);
            self.decor = decor;
            dirty = dirty.max(Dirty::Paint);
        }
        if input.disabled != self.input.disabled {
            dirty = dirty.max(Dirty::Paint);
        }
        self.input = input;
        self.a11y = a11y;

        dirty = dirty.max(reconcile(&mut self.children, children, ids, env));
        if dirty == Dirty::Layout {
            self.cache.clear();
        }
        dirty
    }

    fn refresh_eff(&mut self) {
        self.eff = self.target;
        if let Some(a) = &self.anim.width {
            self.eff.width = Len::Px(a.value());
        }
        if let Some(a) = &self.anim.height {
            self.eff.height = Len::Px(a.value());
        }
    }

    fn retarget_decor(&mut self, new: &Decor, env: &Env) {
        if new.fill != self.decor.fill {
            self.anim.fill = self.transition.map(|spring| {
                let mut t = SpringValue::new(0.0, spring);
                t.set_target(1.0);
                FillAnim { from: self.base_fill(&env.theme.palette), t }
            });
        }
        if new.opacity != self.anim.opacity.target() {
            match self.transition {
                Some(spring) => {
                    self.anim.opacity.spring = spring;
                    self.anim.opacity.set_target(new.opacity);
                }
                None => self.anim.opacity.snap_to(new.opacity),
            }
        }
    }

    /// The fill as it is now, part way through a transition if one is running.
    pub fn base_fill(&self, palette: &Palette) -> Color {
        let to = self.decor.fill.resolve(palette);
        match &self.anim.fill {
            Some(a) => mix(a.from, to, a.t.value().clamp(0.0, 1.0)),
            None => to,
        }
    }

    pub fn focus_mode(&self) -> Focus {
        self.input.focus.unwrap_or_else(|| self.widget.focusable())
    }

    /// The content box of a border box, inside the padding.
    pub fn content_rect(&self, border_box: Rect) -> Rect {
        let p = self.eff.padding;
        border_box.deflate(Insets::new(p.top, p.right, p.bottom, p.left))
    }

    /// True when `p` is on the node and not cut away by an ancestor.
    pub fn hit_self(&self, p: Point) -> bool {
        self.rect.contains(p) && self.clip.is_none_or(|c| c.contains(p))
    }

    /// The scrollbar thumb on `axis`, if the node scrolls that way.
    pub fn bar(&self, axis: Axis) -> Option<Bar> {
        let max = self.scroll.max_along(axis);
        if self.eff.overflow != Overflow::Scroll || max <= 0.0 {
            return None;
        }
        let (view, offset) = match axis {
            Axis::X => (self.rect.w, self.scroll.offset().x),
            Axis::Y => (self.rect.h, self.scroll.offset().y),
        };
        let track = view - 2.0 * BAR_INSET;
        if track <= 0.0 {
            return None;
        }
        let len = (track * view / (view + max)).clamp(BAR_MIN.min(track), track);
        let free = track - len;
        let pos = if free > 0.0 { free * offset / max } else { 0.0 };
        let thick = self.scroll.thick.value().clamp(0.0, 1.0);
        let w = BAR_WIDTH + (BAR_WIDTH_HOT - BAR_WIDTH) * thick;
        let thumb = match axis {
            Axis::Y => rect(self.rect.right() - BAR_INSET - w, self.rect.y + BAR_INSET + pos, w, len),
            Axis::X => rect(self.rect.x + BAR_INSET + pos, self.rect.bottom() - BAR_INSET - w, len, w),
        };
        Some(Bar { thumb, free, max })
    }

    /// Turns the positions layout left behind (relative to each parent) into window coordinates, and
    /// works out what each node is clipped to and how far scroll containers can scroll.
    pub fn arrange(&mut self, origin: Point, clip: Option<Rect>) {
        if self.eff.hidden {
            self.collapse();
            return;
        }
        let at = pt(origin.x + self.layout.origin.x, origin.y + self.layout.origin.y);
        self.rect = Rect::from_origin_size(at, self.layout.size);
        self.clip = clip;
        if self.eff.overflow == Overflow::Scroll {
            let (c, s) = (self.layout.content, self.layout.size);
            self.scroll.set_max(vec2((c.w - s.w).max(0.0), (c.h - s.h).max(0.0)));
        } else if self.scroll.max != Vec2::ZERO {
            self.scroll.set_max(Vec2::ZERO);
        }
        let child_clip = if self.eff.overflow == Overflow::Visible {
            clip
        } else {
            let own = rect(at.x, at.y, 0.0, 0.0);
            Some(match clip {
                Some(c) => c.intersection(self.rect).unwrap_or(own),
                None => self.rect,
            })
        };
        let off = self.scroll.offset();
        let base = pt(self.rect.x - off.x, self.rect.y - off.y);
        for child in &mut self.children {
            child.arrange(base, child_clip);
        }
    }

    fn collapse(&mut self) {
        self.rect = Rect::ZERO;
        self.clip = Some(Rect::ZERO);
        for child in &mut self.children {
            child.collapse();
        }
    }

    /// Advances every spring and widget animation by `dt` seconds. Returns true if something in the
    /// subtree now needs a new layout.
    pub fn tick(&mut self, dt: f32, ts: &mut TickState) -> bool {
        let mut layout = false;

        advance(&mut self.anim.hover, dt, ts);
        advance(&mut self.anim.press, dt, ts);
        advance(&mut self.anim.focus, dt, ts);
        advance(&mut self.anim.drop, dt, ts);
        advance(&mut self.anim.opacity, dt, ts);
        if let Some(fill) = self.anim.fill.as_mut() {
            advance(&mut fill.t, dt, ts);
            if fill.t.is_settled() {
                self.anim.fill = None;
            }
        }

        let mut sized = false;
        for slot in [&mut self.anim.width, &mut self.anim.height] {
            if let Some(a) = slot.as_mut() {
                sized |= advance(a, dt, ts);
                if a.is_settled() {
                    *slot = None;
                    sized = true;
                }
            }
        }
        if sized {
            self.refresh_eff();
            layout = true;
        }

        if self.eff.overflow == Overflow::Scroll {
            let moved_x = advance(&mut self.scroll.x, dt, ts);
            let moved_y = advance(&mut self.scroll.y, dt, ts);
            if moved_x || moved_y {
                ts.arrange = true;
                self.scroll.poke();
            }
            self.scroll.idle += dt;
            let has_bars = self.scroll.max.x > 0.0 || self.scroll.max.y > 0.0;
            let show = has_bars && self.scroll.idle < BAR_HOLD;
            self.scroll.bars.set_target(if show { 1.0 } else { 0.0 });
            advance(&mut self.scroll.bars, dt, ts);
            advance(&mut self.scroll.thick, dt, ts);
            if show {
                ts.moving = true;
            }
        }

        let focused = ts.focus == Some(self.id);
        let mut cx = TickCx {
            env: ts.env,
            time: ts.time,
            window_focused: ts.window_focused,
            focused,
            layout: false,
            paint: false,
            wake: None,
        };
        if self.widget.tick(&mut *self.state, &mut cx, dt) {
            ts.moving = true;
            ts.paint = true;
        }
        layout |= cx.layout;
        ts.paint |= cx.paint;
        if let Some(w) = cx.wake {
            ts.wake = Some(ts.wake.map_or(w, |old| old.min(w)));
        }

        for child in &mut self.children {
            layout |= child.tick(dt, ts);
        }
        if layout {
            self.cache.clear();
            ts.layout = true;
        }
        layout
    }
}

/// Matches new child elements to the nodes already there: by key when they have one, else by position
/// and widget type. Matches keep their state; the rest are built fresh, and nodes nobody claimed are
/// dropped.
fn reconcile<M: 'static>(old: &mut Vec<Node<M>>, new: Vec<Element<M>>, ids: &mut Ids, env: &Env) -> Dirty {
    let before: Vec<NodeId> = old.iter().map(|n| n.id).collect();
    let mut pool: Vec<Option<Node<M>>> = std::mem::take(old).into_iter().map(Some).collect();
    let by_key: HashMap<Key, usize> =
        pool.iter().enumerate().filter_map(|(i, n)| n.as_ref().and_then(|n| n.key.clone()).map(|k| (k, i))).collect();

    let mut dirty = Dirty::None;
    let mut out: Vec<Node<M>> = Vec::with_capacity(new.len());
    for (i, el) in new.into_iter().enumerate() {
        let kind = el.widget.widget_type();
        let same_kind = |slot: &Option<Node<M>>| slot.as_ref().is_some_and(|n| n.widget.widget_type() == kind);
        let found = match &el.base.key {
            Some(key) => by_key.get(key).copied().filter(|&j| same_kind(&pool[j])),
            None => (i < pool.len() && same_kind(&pool[i]) && pool[i].as_ref().is_some_and(|n| n.key.is_none())).then_some(i),
        };
        match found.and_then(|j| pool[j].take()) {
            Some(mut node) => {
                dirty = dirty.max(node.update(el, ids, env));
                out.push(node);
            }
            None => {
                out.push(Node::build(el, ids));
                dirty = Dirty::Layout;
            }
        }
    }
    if pool.iter().any(Option::is_some) || out.iter().map(|n| n.id).ne(before.iter().copied()) {
        dirty = Dirty::Layout;
    }
    *old = out;
    dirty
}

pub(crate) fn node_at<'a, M: 'static>(root: &'a Node<M>, path: &[usize]) -> &'a Node<M> {
    path.iter().fold(root, |n, &i| &n.children[i])
}

pub(crate) fn node_at_mut<'a, M: 'static>(root: &'a mut Node<M>, path: &[usize]) -> &'a mut Node<M> {
    let mut node = root;
    for &i in path {
        node = &mut node.children[i];
    }
    node
}

/// The child indices from the root down to the node with this id.
pub(crate) fn path_of<M: 'static>(root: &Node<M>, id: NodeId) -> Option<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, id: NodeId, path: &mut Vec<usize>) -> bool {
        if node.id == id {
            return true;
        }
        for (i, child) in node.children.iter().enumerate() {
            path.push(i);
            if walk(child, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(root, id, &mut path).then_some(path)
}

/// Every node on the way down `path`, root first.
pub(crate) fn chain<'a, M: 'static>(root: &'a Node<M>, path: &[usize]) -> Vec<&'a Node<M>> {
    let mut nodes = Vec::with_capacity(path.len() + 1);
    let mut node = root;
    nodes.push(node);
    for &i in path {
        node = &node.children[i];
        nodes.push(node);
    }
    nodes
}
