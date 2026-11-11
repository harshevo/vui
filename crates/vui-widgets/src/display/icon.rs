//! Icons as widgets, and the one way every widget draws an icon.

use vui_core::{Fill, ImageCmd, Rect, Sampling, Size, Tone, size};
use vui_icons::{Icon, SPINNER_FRAMES, Weight, spinner};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, PaintCx, Role, Widget, builder};

/// How big an icon is when nothing says otherwise.
pub const ICON_SIZE: f32 = 16.0;

/// Seconds a spinner takes to go round.
pub const SPINNER_PERIOD: f32 = 0.9;

/// Where a stencil goes in `at`: a square of whole device pixels, centred on the pixel grid, and its
/// side in device pixels.
fn stencil_in(cx: &PaintCx, at: Rect) -> (Rect, u32) {
    let scale = cx.env.scale;
    let px = (at.w.min(at.h) * scale).round().max(1.0);
    let side = px / scale;
    let snap = |v: f32| (v * scale).round() / scale;
    (Rect::new(snap(at.x + (at.w - side) / 2.0), snap(at.y + (at.h - side) / 2.0), side, side), px as u32)
}

/// Draws `icon` as a stencil in `fill`, as large as fits in `at` and centred in it. The stencil is
/// drawn at a whole number of device pixels and placed on the pixel grid, so its lines stay crisp.
pub fn paint_icon(cx: &mut PaintCx, icon: Icon, at: Rect, fill: impl Into<Fill>, weight: Weight) {
    let (dest, px) = stencil_in(cx, at);
    // The element's opacity is applied by `cx.image`, so the tint is the plain colour.
    let tint = fill.into().resolve(cx.palette());
    let id = icon.image(cx.env.images(), px, weight);
    cx.image(ImageCmd::new(id, dest).mask(tint).sampling(Sampling::Nearest));
}

/// Draws the spinner at the frame the clock is on: a ring with a gap that turns once every
/// [`SPINNER_PERIOD`] seconds. The caller keeps frames coming while it is on screen.
pub fn paint_spinner(cx: &mut PaintCx, at: Rect, fill: impl Into<Fill>) {
    let (dest, px) = stencil_in(cx, at);
    let frame = ((cx.time / SPINNER_PERIOD).fract() * SPINNER_FRAMES as f32) as u32 % SPINNER_FRAMES;
    let tint = fill.into().resolve(cx.palette());
    let id = spinner::image(cx.env.images(), px, frame, Weight::Regular);
    cx.image(ImageCmd::new(id, dest).mask(tint).sampling(Sampling::Nearest));
}

/// The widget behind [`Icon`](crate::icon).
pub struct IconWidget {
    icon: Icon,
    color: Fill,
    weight: Weight,
}

impl<M: 'static> Widget<M> for IconWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &IconWidget, _state: &mut ()) -> Dirty {
        if self.icon != old.icon || self.weight != old.weight || self.color != old.color { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut (), _env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let side = known.x.or(known.y).unwrap_or(ICON_SIZE);
        size(known.x.unwrap_or(side), known.y.unwrap_or(side))
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        paint_icon(cx, self.icon, cx.content, self.color, self.weight);
    }

    /// Decorative unless the app names it.
    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::None);
    }
}

/// An icon from the set, drawn in a colour of the theme.
pub struct IconEl<M: 'static> {
    base: Base<M>,
    widget: IconWidget,
}

builder!(IconEl<M> => IconWidget);

impl<M: 'static> IconEl<M> {
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.widget.weight = weight;
        self
    }
}

/// An icon, 16 points square unless sized: `icon(set::SEARCH).square(20.0)`.
pub fn icon<M: 'static>(icon: Icon) -> IconEl<M> {
    IconEl { base: Base::new(), widget: IconWidget { icon, color: Tone::Text2.into(), weight: Weight::Regular } }
}
