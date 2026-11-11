//! Tooltips: a short line of text that appears under an element after the pointer has rested on it
//! (`Builder::tooltip`). The runtime keeps the timing and draws the bubble above everything, inside
//! the window, so neither the app nor the widget keeps state for it.

use std::rc::Rc;

use vui_core::{Quad, Rect, Scene, Size, TextStyle, TextWrap, rect};

use crate::key::NodeId;
use crate::node::Node;
use crate::widget::Env;

/// How long the pointer rests before the tip shows.
pub(crate) const TIP_DELAY: f32 = 0.6;
/// Space between the element and the bubble, and between the bubble and the window's edge.
const GAP: f32 = 6.0;
const EDGE: f32 = 8.0;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 4.0;
/// Text wider than this wraps.
const MAX_WIDTH: f32 = 280.0;

/// Which element's tip is pending or showing.
#[derive(Default)]
pub(crate) struct Tip {
    pub node: Option<NodeId>,
    pub since: f32,
    pub shown: bool,
    /// A press or a key put it away; it stays away until the pointer moves to something else.
    pub blocked: bool,
}

impl Tip {
    /// The pointer is now over `node` (the innermost element with a tip, if any), at `time`.
    /// Returns true if a new tip should be waited for.
    pub fn hover(&mut self, node: Option<NodeId>, time: f32) -> bool {
        if node == self.node {
            return false;
        }
        *self = Tip { node, since: time, shown: false, blocked: false };
        node.is_some()
    }

    /// Hides the tip until the pointer moves elsewhere. Returns true if it was showing.
    pub fn block(&mut self) -> bool {
        self.blocked = true;
        std::mem::take(&mut self.shown)
    }

    /// Shows the tip once the pointer has rested long enough. Returns true if it just appeared.
    pub fn tick(&mut self, time: f32) -> bool {
        if self.node.is_some() && !self.shown && !self.blocked && time - self.since >= TIP_DELAY - 1e-4 {
            self.shown = true;
            return true;
        }
        false
    }
}

/// Where the bubble goes for an element at `anchor`: under it, or over it when there is no room
/// below, and moved sideways to stay inside the window.
pub(crate) fn place(anchor: Rect, bubble: Size, window: Size) -> Rect {
    let x = (anchor.x + (anchor.w - bubble.w) / 2.0).clamp(EDGE, (window.w - EDGE - bubble.w).max(EDGE));
    let below = anchor.y + anchor.h + GAP;
    let y = if below + bubble.h + EDGE > window.h && anchor.y - GAP - bubble.h >= EDGE { anchor.y - GAP - bubble.h } else { below };
    rect(x, y, bubble.w, bubble.h)
}

/// Draws the showing tip, if any, on top of the scene.
pub(crate) fn paint<M: 'static>(tip: &Tip, root: &Node<M>, env: &Env, window: Size, scene: &mut Scene) {
    if !tip.shown {
        return;
    }
    let Some(node) = tip.node.and_then(|id| crate::ui::find_id(root, id)) else { return };
    let Some(text) = node.input.tooltip.as_deref() else { return };
    let palette = &env.theme.palette;
    let mut style = TextStyle::from_token(env.theme.types.small);
    style.wrap = TextWrap::Words;
    let layout: Rc<_> = env.res.fonts.layout_cached(text, &style, Some(MAX_WIDTH));
    let size = layout.size();
    let bubble = place(node.rect, vui_core::size(size.w + PAD_X * 2.0, size.h + PAD_Y * 2.0), window);
    let bubble = bubble.snap(env.scale);
    scene.quad(Quad::new(bubble).fill(palette.text.mul_alpha(0.92)).radii(vui_core::Corners::all(6.0)));
    scene.text(layout, vui_core::pt(bubble.x + PAD_X, bubble.y + PAD_Y), palette.bg, None);
}
