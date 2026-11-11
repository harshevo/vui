//! Effects drawn by the renderer's built-in shader pass: soft shadows under raised surfaces, a
//! shimmer that sweeps across something at work, and a glow that breathes around a selection.
//!
//! Each is a [`CustomCmd`] for [`PASS`], so it is layered with quads, images and text in paint order.
//! The data is laid out here and read by `vui-render`'s effects shader; keep the two in step.

use crate::{Color, CustomCmd, Rect};

/// The name of the renderer's built-in effects pass.
pub const PASS: &str = "vui.effects";

const SHADOW: f32 = 0.0;
const SHIMMER: f32 = 1.0;
const GLOW: f32 = 2.0;

/// A shadow under a rounded box: Gaussian blur `blur` (points, about two standard deviations),
/// dropped `y` points, grown by `spread`, black at `alpha`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shadow {
    pub blur: f32,
    pub y: f32,
    pub spread: f32,
    pub alpha: f32,
}

impl Shadow {
    /// Barely lifted: rows and tiles.
    pub const SOFT: Shadow = Shadow { blur: 8.0, y: 2.0, spread: 0.0, alpha: 0.06 };
    /// A card above the page.
    pub const RAISED: Shadow = Shadow { blur: 24.0, y: 8.0, spread: -2.0, alpha: 0.10 };
    /// A panel or popover floating over everything.
    pub const FLOATING: Shadow = Shadow { blur: 48.0, y: 16.0, spread: -4.0, alpha: 0.18 };

    /// How far past the box the shadow reaches.
    pub fn reach(&self) -> f32 {
        self.blur * 1.5 + self.y.abs() + self.spread.max(0.0)
    }

    /// The same shadow with its strength scaled, for dark surfaces or a fade.
    pub fn times(self, k: f32) -> Shadow {
        Shadow { alpha: (self.alpha * k).clamp(0.0, 1.0), ..self }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd(kind: f32, area: Rect, shape: Rect, radius: f32, color: Color, a: f32, b: f32, c: f32, time: f32) -> CustomCmd {
    CustomCmd::new(PASS, area, vec![kind, shape.x, shape.y, shape.w, shape.h, radius, color.r, color.g, color.b, color.a, a, b, c, time])
}

/// The shadow of the rounded box `shape`.
pub fn shadow(shape: Rect, radius: f32, s: Shadow) -> CustomCmd {
    let reach = s.reach();
    cmd(SHADOW, shape.inflate(reach, reach), shape, radius, Color::rgba(0.0, 0.0, 0.0, s.alpha), s.blur, s.y, s.spread, 0.0)
}

/// A band of `color` that sweeps across the rounded box `shape` every `period` seconds.
pub fn shimmer(shape: Rect, radius: f32, color: Color, period: f32, time: f32) -> CustomCmd {
    cmd(SHIMMER, shape, shape, radius, color, period.max(0.1), (shape.w * 0.35).clamp(24.0, 160.0), 0.0, time)
}

/// A ring of `color` `width` points wide around `shape`, glowing `blur` points outward and
/// breathing once every `period` seconds (0: steady).
pub fn glow(shape: Rect, radius: f32, color: Color, width: f32, blur: f32, period: f32, time: f32) -> CustomCmd {
    let reach = blur * 3.0 + width;
    cmd(GLOW, shape.inflate(reach, reach), shape, radius, color, width, blur, period, time)
}
