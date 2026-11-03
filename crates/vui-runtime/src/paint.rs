//! Painting the tree into a [`Scene`].
//!
//! Each node paints its own box first (fill, hover and press fades, border), then its widget, then its
//! children, then whatever the widget wants over them, and last the focus ring and scrollbars.

use vui_core::{Border, Corners, Quad, Rect, Scene};

use crate::key::NodeId;
use crate::node::Node;
use crate::style::{Axis, Overflow};
use crate::widget::{Env, PaintCx};

/// Disabled elements are drawn at this fraction of their opacity.
const DISABLED_ALPHA: f32 = 0.45;
/// Space between an element's edge and its focus ring, and the ring's thickness.
const RING_GAP: f32 = 1.0;
const RING_WIDTH: f32 = 2.0;

/// What painting needs to know that is not in the tree.
pub(crate) struct PaintEnv<'a> {
    pub env: &'a Env,
    pub focus: Option<NodeId>,
    pub window_focused: bool,
    pub time: f32,
}

/// What a node looks like right now, shared by the calls into its widget.
struct Look {
    rect: Rect,
    content: Rect,
    exact: Rect,
    clip: Option<Rect>,
    radii: Corners,
    smooth: f32,
    alpha: f32,
    hover: f32,
    press: f32,
    focus: f32,
    focused: bool,
    disabled: bool,
}

impl Look {
    fn cx<'a>(&self, scene: &'a mut Scene, pe: &'a PaintEnv) -> PaintCx<'a> {
        PaintCx {
            scene,
            env: pe.env,
            rect: self.rect,
            content: self.content,
            exact: self.exact,
            clip: self.clip,
            radii: self.radii,
            smooth: self.smooth,
            alpha: self.alpha,
            hover: self.hover,
            press: self.press,
            focus: self.focus,
            focused: self.focused,
            window_focused: pe.window_focused,
            disabled: self.disabled,
            time: pe.time,
        }
    }
}

/// Paints the tree, then the floating elements above it in the order they come, each with what
/// floats inside it above that.
pub(crate) fn paint<M: 'static>(root: &Node<M>, scene: &mut Scene, pe: &PaintEnv, inherited: f32) {
    let mut floats = Vec::new();
    paint_node(root, scene, pe, inherited, &mut floats);
    while !floats.is_empty() {
        let mut above = Vec::new();
        for (node, alpha) in floats {
            paint_node(node, scene, pe, alpha, &mut above);
        }
        floats = above;
    }
}

fn paint_node<'a, M: 'static>(
    node: &'a Node<M>,
    scene: &mut Scene,
    pe: &PaintEnv,
    inherited: f32,
    floats: &mut Vec<(&'a Node<M>, f32)>,
) {
    if node.eff.hidden {
        return;
    }
    let env = pe.env;
    let theme = &*env.theme;
    let palette = &theme.palette;
    let disabled = node.input.disabled;
    let alpha = inherited * node.anim.opacity.value().clamp(0.0, 1.0) * if disabled { DISABLED_ALPHA } else { 1.0 };
    if alpha <= 0.0 {
        return;
    }

    let rect = node.rect.snap(env.scale);
    let clips = node.eff.overflow != Overflow::Visible;
    if clips && (rect.is_empty() || node.clip.is_some_and(|c| !c.intersects(rect))) {
        return;
    }

    let radii = node.decor.radius.resolve(theme);
    let smooth = node.decor.radius.smooth(theme);
    // What the node paints inside itself is cut at its own edge too, if it clips.
    let inner_clip = if clips { Some(node.clip.map_or(rect, |c| c.intersection(rect).unwrap_or(Rect::ZERO))) } else { node.clip };
    let look = Look {
        rect,
        content: node.content_rect(rect),
        exact: node.content_rect(node.rect),
        clip: inner_clip,
        radii,
        smooth,
        alpha,
        hover: node.anim.hover.value().clamp(0.0, 1.0),
        press: node.anim.press.value().clamp(0.0, 1.0),
        focus: node.anim.focus.value().clamp(0.0, 1.0),
        focused: pe.focus == Some(node.id),
        disabled,
    };

    let mut fill = node.base_fill(palette);
    for (over, amount) in [
        (node.decor.hover_fill, look.hover),
        (node.decor.press_fill, look.press),
        (node.decor.drop_fill, node.anim.drop.value().clamp(0.0, 1.0)),
    ] {
        if amount > 0.0 && !over.is_none() {
            fill = over.resolve(palette).mul_alpha(amount).over(fill);
        }
    }
    // Nothing of this node's own is on screen: skip its box and its widget, but its children may be.
    let on_screen = node.clip.is_none_or(|c| c.intersects(rect));
    if on_screen {
        let fill = fill.mul_alpha(alpha);
        if let Some(shadow) = node.decor.shadow {
            let k = if theme.mode == vui_core::Mode::Dark { 2.5 } else { 1.0 };
            let mut cmd = vui_core::effects::shadow(rect, radii.tl.max(radii.br), shadow.times(k));
            cmd.clip = node.clip;
            cmd.opacity = alpha;
            scene.push(vui_core::Command::Custom(cmd));
        }
        let border = node.decor.border.map(|(width, color)| Border { width, color: color.resolve(palette).mul_alpha(alpha) });
        if fill.a > 0.0 || border.is_some_and(|b| b.width > 0.0 && b.color.a > 0.0) {
            scene.quad(Quad { rect, radii, smooth, fill, border, clip: node.clip });
        }
        node.widget.paint(&*node.state, &mut look.cx(scene, pe));
    }
    for child in &node.children {
        if child.eff.float {
            floats.push((child, alpha));
        } else {
            paint_node(child, scene, pe, alpha, floats);
        }
    }
    if !on_screen {
        return;
    }
    node.widget.paint_over(&*node.state, &mut look.cx(scene, pe));

    if node.decor.focus_ring && look.focus > 0.01 {
        let grow = RING_GAP + RING_WIDTH;
        let ring = Corners::new(radii.tl + grow, radii.tr + grow, radii.br + grow, radii.bl + grow);
        let color = palette.accent.mul_alpha(look.focus * alpha);
        scene.quad(Quad::new(rect.inflate(grow, grow)).radii(ring).smooth(smooth).border(RING_WIDTH, color).clip(node.clip));
    }

    if node.eff.overflow == Overflow::Scroll {
        let visible = node.scroll.bars.value().clamp(0.0, 1.0);
        if visible > 0.01 {
            let color = palette.text_3.mul_alpha(0.8 * visible * alpha);
            for axis in [Axis::Y, Axis::X] {
                if let Some(bar) = node.bar(axis) {
                    let t = bar.thumb;
                    scene.quad(Quad::new(t).fill(color).radii(t.w.min(t.h) / 2.0).clip(node.clip));
                }
            }
        }
    }
}
