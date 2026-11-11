//! Shaded fills, drawn by the renderer's built-in shading pass: linear, radial and conic gradients
//! (as fills or as borders), a slowly moving aurora of soft colour, a beam of light travelling round
//! an edge, a spotlight, and film grain.
//!
//! Colours blend in Oklab, so a gradient from blue to yellow passes through clean greens rather than
//! grey, as in CSS's `in oklab`. Each fill is a [`CustomCmd`] for [`PASS`], layered with everything
//! else in paint order. The data is laid out here and read by `vui-render`'s shading shader and its
//! CPU twin; keep the three in step.

use crate::theme::{Fill, Palette};
use crate::{Color, CustomCmd, Point, Rect};

/// The name of the renderer's built-in shading pass.
pub const PASS: &str = "vui.shading";

/// The most colour stops a gradient carries; more are dropped.
pub const MAX_STOPS: usize = 4;

pub const LINEAR: f32 = 0.0;
pub const RADIAL: f32 = 1.0;
pub const CONIC: f32 = 2.0;
pub const AURORA: f32 = 3.0;
pub const BEAM: f32 = 4.0;
pub const SPOTLIGHT: f32 = 5.0;
pub const GRAIN: f32 = 6.0;

/// How long the data of a shading command is.
pub const DATA_LEN: usize = 33;

/// Where a gradient's colours run.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GradientShape {
    /// Toward `angle` degrees, as in CSS: 0 runs bottom to top, 90 left to right, 180 top to bottom.
    Linear { angle: f32 },
    /// Outward from `centre` (fractions of the box) to `radius` (a fraction of the box's longer side).
    Radial { centre: (f32, f32), radius: f32 },
    /// Round `centre`, starting at `from` degrees (0 points right, turning clockwise).
    Conic { centre: (f32, f32), from: f32 },
}

/// A gradient of up to four colours, each a tone or a colour, so it follows the theme.
///
/// ```
/// use vui_core::shading::Gradient;
/// use vui_core::{Color, Tone};
///
/// let sunrise = Gradient::linear(135.0, [Color::hex(0xf97316), Color::hex(0xec4899)]);
/// let brand = Gradient::radial([Tone::Accent, Tone::Purple]).at(0.2, 0.0).radius(1.2);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub shape: GradientShape,
    pub stops: Vec<(f32, Fill)>,
}

fn spaced<F: Into<Fill>>(colors: impl IntoIterator<Item = F>) -> Vec<(f32, Fill)> {
    let colors: Vec<Fill> = colors.into_iter().map(Into::into).take(MAX_STOPS).collect();
    let last = colors.len().saturating_sub(1).max(1) as f32;
    colors.into_iter().enumerate().map(|(i, c)| (i as f32 / last, c)).collect()
}

impl Gradient {
    /// Colours evenly spaced toward `angle` degrees, as in CSS: `linear(135.0, …)` runs from the top
    /// left to the bottom right.
    pub fn linear<F: Into<Fill>>(angle: f32, colors: impl IntoIterator<Item = F>) -> Gradient {
        Gradient { shape: GradientShape::Linear { angle }, stops: spaced(colors) }
    }

    /// Colours evenly spaced outward from the centre to the far edge.
    pub fn radial<F: Into<Fill>>(colors: impl IntoIterator<Item = F>) -> Gradient {
        Gradient { shape: GradientShape::Radial { centre: (0.5, 0.5), radius: 0.71 }, stops: spaced(colors) }
    }

    /// Colours evenly spaced round the centre. Repeat the first at the end for a seamless ring.
    pub fn conic<F: Into<Fill>>(colors: impl IntoIterator<Item = F>) -> Gradient {
        Gradient { shape: GradientShape::Conic { centre: (0.5, 0.5), from: -90.0 }, stops: spaced(colors) }
    }

    /// Moves a radial or conic gradient's centre, in fractions of the box (0.5, 0.5 is the middle).
    pub fn at(mut self, x: f32, y: f32) -> Gradient {
        match &mut self.shape {
            GradientShape::Radial { centre, .. } | GradientShape::Conic { centre, .. } => *centre = (x, y),
            GradientShape::Linear { .. } => {}
        }
        self
    }

    /// How far a radial gradient reaches, as a fraction of the box's longer side.
    pub fn radius(mut self, r: f32) -> Gradient {
        if let GradientShape::Radial { radius, .. } = &mut self.shape {
            *radius = r.max(0.01);
        }
        self
    }

    /// Where a conic gradient starts, in degrees.
    pub fn from(mut self, degrees: f32) -> Gradient {
        if let GradientShape::Conic { from, .. } = &mut self.shape {
            *from = degrees;
        }
        self
    }

    /// Places the stops at `positions` (0 to 1) instead of evenly.
    pub fn positions(mut self, positions: &[f32]) -> Gradient {
        for (stop, &p) in self.stops.iter_mut().zip(positions) {
            stop.0 = p.clamp(0.0, 1.0);
        }
        self
    }

    /// The colours for this theme.
    pub fn ramp(&self, palette: &Palette) -> Ramp {
        Ramp::new(self.stops.iter().map(|(p, f)| (*p, f.resolve(palette))))
    }
}

/// Up to four resolved colours and where they sit, 0 to 1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ramp {
    pub colors: [Color; MAX_STOPS],
    pub positions: [f32; MAX_STOPS],
    pub count: usize,
}

impl Ramp {
    pub fn new(stops: impl IntoIterator<Item = (f32, Color)>) -> Ramp {
        let mut ramp = Ramp { colors: [Color::TRANSPARENT; MAX_STOPS], positions: [0.0; MAX_STOPS], count: 0 };
        for (p, c) in stops.into_iter().take(MAX_STOPS) {
            ramp.colors[ramp.count] = c;
            ramp.positions[ramp.count] = p.clamp(0.0, 1.0);
            ramp.count += 1;
        }
        // Keep positions in order, so the shader's search never runs backwards.
        for i in 1..ramp.count {
            ramp.positions[i] = ramp.positions[i].max(ramp.positions[i - 1]);
        }
        ramp
    }

    /// The same colours, evenly spaced.
    pub fn even(colors: &[Color]) -> Ramp {
        let last = colors.len().min(MAX_STOPS).saturating_sub(1).max(1) as f32;
        Ramp::new(colors.iter().enumerate().map(|(i, &c)| (i as f32 / last, c)))
    }

    /// Every colour faded by `k`.
    pub fn mul_alpha(mut self, k: f32) -> Ramp {
        for c in &mut self.colors {
            *c = c.mul_alpha(k);
        }
        self
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd(kind: f32, area: Rect, shape: Rect, radius: f32, p: [f32; 4], ramp: &Ramp, time: f32, ring: f32) -> CustomCmd {
    let mut data = Vec::with_capacity(DATA_LEN);
    data.extend_from_slice(&[kind, shape.x, shape.y, shape.w, shape.h, radius]);
    data.extend_from_slice(&p);
    data.push(ramp.count as f32);
    data.extend_from_slice(&ramp.positions);
    for c in ramp.colors {
        data.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    }
    data.extend_from_slice(&[time, ring]);
    CustomCmd::new(PASS, area, data)
}

fn shape_params(shape: GradientShape) -> (f32, [f32; 4]) {
    match shape {
        // CSS angles turn clockwise from up; the shader wants the direction's angle from the x axis.
        GradientShape::Linear { angle } => (LINEAR, [(angle - 90.0).to_radians(), 0.0, 0.0, 0.0]),
        GradientShape::Radial { centre, radius } => (RADIAL, [centre.0, centre.1, radius, 0.0]),
        GradientShape::Conic { centre, from } => (CONIC, [centre.0, centre.1, from.to_radians(), 0.0]),
    }
}

/// The rounded box `shape` filled with `gradient`.
pub fn fill(shape: Rect, radius: f32, gradient: &Gradient, palette: &Palette) -> CustomCmd {
    let (kind, p) = shape_params(gradient.shape);
    cmd(kind, shape, shape, radius, p, &gradient.ramp(palette), 0.0, 0.0)
}

/// A border `width` points wide inside the edge of `shape`, coloured by `gradient`.
pub fn border(shape: Rect, radius: f32, width: f32, gradient: &Gradient, palette: &Palette) -> CustomCmd {
    let (kind, p) = shape_params(gradient.shape);
    cmd(kind, shape, shape, radius, p, &gradient.ramp(palette), 0.0, width.max(0.0))
}

/// Soft blobs of the ramp's colours drifting over the first one, a full cycle every `period`
/// seconds. `size` is a blob's reach as a fraction of the box's longer side; `grain` adds film grain
/// (0.03 is subtle).
pub fn aurora(shape: Rect, radius: f32, ramp: &Ramp, period: f32, size: f32, grain: f32, time: f32) -> CustomCmd {
    cmd(AURORA, shape, shape, radius, [period.max(0.5), size.max(0.05), grain.max(0.0), 0.0], ramp, time, 0.0)
}

/// A beam of light `width` points wide running round the edge of `shape` once every `period`
/// seconds, its tail `length` of the way round (0 to 1) and fading through the ramp to its head.
pub fn beam(shape: Rect, radius: f32, width: f32, ramp: &Ramp, period: f32, length: f32, time: f32) -> CustomCmd {
    cmd(BEAM, shape, shape, radius, [width.max(0.5), period.max(0.2), length.clamp(0.02, 1.0), 0.0], ramp, time, 0.0)
}

/// A soft circle of `color` `reach` points across, centred `at` (relative to the box), cut to the
/// rounded box `shape`: the light under the pointer on a spotlight card.
pub fn spotlight(shape: Rect, radius: f32, at: Point, reach: f32, color: Color) -> CustomCmd {
    cmd(SPOTLIGHT, shape, shape, radius, [at.x, at.y, reach.max(1.0), 0.0], &Ramp::new([(0.0, color)]), 0.0, 0.0)
}

/// Film grain over the rounded box `shape`: light and dark specks, `amount` strong (0.03 is subtle).
pub fn grain(shape: Rect, radius: f32, amount: f32) -> CustomCmd {
    cmd(GRAIN, shape, shape, radius, [amount.max(0.0), 0.0, 0.0, 0.0], &Ramp::new([]), 0.0, 0.0)
}

/// A colour in Oklab, `[L, a, b, alpha]`, from straight sRGB.
pub fn to_oklab(c: Color) -> [f32; 4] {
    let [r, g, b] = c.to_linear_rgb();
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        c.a,
    ]
}

/// Linear-light RGB from Oklab `[L, a, b]`.
pub fn oklab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let l = lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2];
    let m = lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2];
    let s = lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2];
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tone, rect};

    #[test]
    fn oklab_goes_there_and_back() {
        for c in [Color::hex(0x4f5bd5), Color::hex(0xf97316), Color::WHITE, Color::BLACK] {
            let lab = to_oklab(c);
            let back = oklab_to_linear([lab[0], lab[1], lab[2]]);
            let want = c.to_linear_rgb();
            for i in 0..3 {
                assert!((back[i] - want[i]).abs() < 1e-4, "{c:?}: {back:?} vs {want:?}");
            }
        }
        assert!((to_oklab(Color::WHITE)[0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn stops_are_spaced_and_capped() {
        let g = Gradient::linear(0.0, [Tone::Accent, Tone::Blue, Tone::Purple, Tone::Orange, Tone::Danger]);
        assert_eq!(g.stops.len(), MAX_STOPS);
        assert_eq!(g.stops.iter().map(|s| s.0).collect::<Vec<_>>(), [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
        let ramp = Ramp::new([(0.6, Color::WHITE), (0.2, Color::BLACK)]);
        assert_eq!(ramp.positions[..2], [0.6, 0.6], "kept in order");
    }

    #[test]
    fn a_command_carries_everything_the_shader_reads() {
        let g = Gradient::conic([Color::WHITE, Color::BLACK]).at(0.25, 0.75).from(0.0);
        let c = border(rect(0.0, 0.0, 10.0, 20.0), 4.0, 2.0, &g, &crate::Theme::light().palette);
        assert_eq!(c.data.len(), DATA_LEN);
        assert_eq!(c.data[0], CONIC);
        assert_eq!(&c.data[6..9], &[0.25, 0.75, 0.0]);
        assert_eq!(c.data[10], 2.0);
        assert_eq!(c.data[32], 2.0, "the ring width");
    }
}
