//! The retained tree: one [`Node`] per element, kept between updates.
//!
//! The app builds a fresh [`Element`] tree after every change. Reconciling it against the nodes the
//! runtime already has keeps each node's widget state, springs, scroll position and layout cache, and
//! reports how much work the change needs: nothing, a repaint, or a new layout.

use std::any::Any;
use std::collections::HashMap;

use vui_core::{Color, Insets, Palette, Point, Rect, Spring, SpringValue, Vec2, pt, rect, vec2};

use crate::access::A11y;
use crate::element::{Base, Decor, Element, Input, Lazy};
use crate::key::{Key, NodeId};
use crate::layout::{Avail, LayoutCache, LayoutNode, NodeLayout, Pair};
use crate::responsive::{self, Rule};
use crate::style::{Axis, Len, Overflow, Position, Style, Thumb};
use crate::widget::{Dirty, Env, Erased, Focus, TickCx};

mod motion;
mod reconcile;

use motion::*;

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
/// The shortest a fixed thumb gets, so it can still be grabbed in a short view.
const BAR_FIXED_MIN: f32 = 16.0;
/// How near the end, in points, still counts as at the end for a container that follows it.
const FOLLOW_SLACK: f32 = 2.0;
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
    /// Top, right, bottom and left insets of an absolute element that is sliding to a new place.
    inset: [Option<SpringValue>; 4],
    /// How far below its place an appearing element still is.
    rise: Option<SpringValue>,
    fill: Option<FillAnim>,
    /// The transform's scale, rotation and translation while they spring to new values.
    pub transform: Option<[SpringValue; 5]>,
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
            inset: [None, None, None, None],
            rise: None,
            fill: None,
            transform: None,
        }
    }

    /// Starts the entrance of an element that has just been built: from transparent and `RISE` below
    /// its place to where it belongs.
    fn appear(&mut self, opacity: f32, spring: Spring) {
        self.opacity = SpringValue::new(0.0, spring);
        self.opacity.set_target(opacity);
        let mut rise = SpringValue::new(RISE, spring);
        rise.set_target(0.0);
        self.rise = Some(rise);
    }
}

/// Points an appearing element starts below its place.
pub(crate) const RISE: f32 = 8.0;

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

    /// Records how far the content can scroll, pulling the position back if the content shrank, and
    /// with `follow`, keeping a position that was at the end there.
    fn set_max(&mut self, max: Vec2, follow: bool) {
        if follow && max.y > self.max.y && self.y.target() >= self.max.y - FOLLOW_SLACK {
            self.y.snap_to(max.y);
        }
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
    /// The element's own style, before its responsive rules.
    pub asked: Style,
    /// Restyles for ranges of window widths (see [`crate::responsive`]).
    pub rules: Vec<Rule>,
    /// What the element asked for at the window's current width: `asked` with the matching rules
    /// applied. `eff` follows it, one step behind while a size is animating.
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
    /// For a focus group: the item inside it that had focus last.
    pub memory: Option<NodeId>,
    /// The width of the box the node was last laid out in, which container rules match against.
    pub room: Option<f32>,
    /// A virtual list's rows, and which of them are built as children.
    pub lazy: Option<(Lazy<M>, std::ops::Range<usize>)>,
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

impl<M: 'static> Node<M> {

    /// The fill as it is now, part way through a transition if one is running.
    pub fn base_fill(&self, palette: &Palette) -> Color {
        let to = self.decor.fill.resolve(palette);
        match &self.anim.fill {
            Some(a) => mix(a.from, to, a.t.value().clamp(0.0, 1.0)),
            None => to,
        }
    }

    /// The transform as drawn right now, part way through a transition or at rest.
    pub fn transform(&self) -> crate::element::Transform {
        match &self.anim.transform {
            Some(springs) => self.decor.transform.with_values(springs.each_ref().map(SpringValue::value)),
            None => self.decor.transform,
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
        let len = match self.eff.thumb {
            Thumb::Proportional => (track * view / (view + max)).clamp(BAR_MIN.min(track), track),
            Thumb::Fixed(fraction) => (track * fraction).clamp(BAR_FIXED_MIN.min(track), track),
        };
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
        let rise = self.anim.rise.as_ref().map_or(0.0, SpringValue::value);
        let at = pt(origin.x + self.layout.origin.x, origin.y + self.layout.origin.y + rise);
        self.rect = Rect::from_origin_size(at, self.layout.size);
        self.clip = clip;
        if self.eff.overflow == Overflow::Scroll {
            let (c, s) = (self.layout.content, self.layout.size);
            self.scroll.set_max(vec2((c.w - s.w).max(0.0), (c.h - s.h).max(0.0)), self.eff.follow_end);
        } else if self.scroll.max != Vec2::ZERO {
            self.scroll.set_max(Vec2::ZERO, false);
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
            if child.eff.position == Position::Fixed {
                // Placed against the window, whatever it sits in.
                child.arrange(Point::ZERO, None);
                continue;
            }
            // A floating element is above everything, so nothing it sits in cuts it off.
            child.arrange(base, if child.eff.float { None } else { child_clip });
        }
    }

    fn collapse(&mut self) {
        self.rect = Rect::ZERO;
        self.clip = Some(Rect::ZERO);
        for child in &mut self.children {
            child.collapse();
        }
    }
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
