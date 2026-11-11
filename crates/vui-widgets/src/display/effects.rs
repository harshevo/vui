//! Boxes that light up what is inside them: a shimmer sweeping across work in progress and a glow
//! ring around something selected or listening. Both fade in and out, and use the renderer's
//! built-in effects pass, so they need nothing from the app.

use vui_core::{Fill, Spring, SpringValue, Tone, effects};
use vui_runtime::{Base, Dirty, Element, PaintCx, TickCx, Widget, builder};

/// Seconds for a shimmer to cross and rest.
const SWEEP: f32 = 1.6;

/// The widget behind [`Shimmer`].
pub struct ShimmerWidget {
    on: bool,
    color: Fill,
    period: f32,
}

/// How far an effect has faded in, from 0 to 1, springing toward on or off.
pub struct EffectState {
    pub(crate) shown: SpringValue,
}

impl EffectState {
    pub(crate) fn new(on: bool) -> EffectState {
        EffectState { shown: SpringValue::new(if on { 1.0 } else { 0.0 }, Spring::SMOOTH) }
    }
}

impl<M: 'static> Widget<M> for ShimmerWidget {
    type State = EffectState;

    fn create_state(&self) -> EffectState {
        EffectState::new(self.on)
    }

    fn update(&self, old: &ShimmerWidget, state: &mut EffectState) -> Dirty {
        state.shown.set_target(if self.on { 1.0 } else { 0.0 });
        if self.color != old.color || self.period != old.period { Dirty::Paint } else { Dirty::None }
    }

    fn paint_over(&self, state: &EffectState, cx: &mut PaintCx) {
        let shown = state.shown.value().clamp(0.0, 1.0);
        if shown <= 0.01 {
            return;
        }
        let color = self.color.resolve(&cx.env.theme.palette).mul_alpha(shown);
        let radius = cx.radii.tl.max(cx.radii.br);
        cx.custom(effects::shimmer(cx.rect, radius, color, self.period, cx.time));
    }

    fn tick(&self, state: &mut EffectState, _cx: &mut TickCx, dt: f32) -> bool {
        let moving = state.shown.step(dt);
        self.on || moving
    }

    fn animating(&self, state: &EffectState) -> bool {
        self.on || !state.shown.is_settled()
    }
}

/// A box whose contents shimmer while `on`: `shimmer(true).child(label("Thinking"))`.
pub struct Shimmer<M: 'static> {
    base: Base<M>,
    widget: ShimmerWidget,
    children: Vec<Element<M>>,
}

builder!(Shimmer<M> => ShimmerWidget, children);

impl<M: 'static> Shimmer<M> {
    /// The band's colour; a translucent white by default, which reads on any surface.
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    /// Seconds per sweep.
    pub fn period(mut self, seconds: f32) -> Self {
        self.widget.period = seconds.max(0.2);
        self
    }
}

pub fn shimmer<M: 'static>(on: bool) -> Shimmer<M> {
    Shimmer { base: Base::new(), widget: ShimmerWidget { on, color: Fill::from(vui_core::Color::rgba(1.0, 1.0, 1.0, 0.35)), period: SWEEP }, children: Vec::new() }
}

/// The widget behind [`Glow`].
pub struct GlowWidget {
    on: bool,
    color: Fill,
    width: f32,
    blur: f32,
    period: f32,
}

impl<M: 'static> Widget<M> for GlowWidget {
    type State = EffectState;

    fn create_state(&self) -> EffectState {
        EffectState::new(self.on)
    }

    fn update(&self, old: &GlowWidget, state: &mut EffectState) -> Dirty {
        state.shown.set_target(if self.on { 1.0 } else { 0.0 });
        let same = self.color == old.color && self.width == old.width && self.blur == old.blur && self.period == old.period;
        if same { Dirty::None } else { Dirty::Paint }
    }

    fn paint(&self, state: &EffectState, cx: &mut PaintCx) {
        let shown = state.shown.value().clamp(0.0, 1.0);
        if shown <= 0.01 {
            return;
        }
        let color = self.color.resolve(&cx.env.theme.palette).mul_alpha(shown);
        let radius = cx.radii.tl.max(cx.radii.br);
        cx.custom(effects::glow(cx.rect, radius, color, self.width, self.blur, self.period, cx.time));
    }

    fn tick(&self, state: &mut EffectState, _cx: &mut TickCx, dt: f32) -> bool {
        let moving = state.shown.step(dt);
        (self.on && self.period > 0.0) || moving
    }

    fn animating(&self, state: &EffectState) -> bool {
        (self.on && self.period > 0.0) || !state.shown.is_settled()
    }
}

/// A box with a ring glowing around its edge while `on`: `glow(listening).rounded(12.0).child(...)`.
pub struct Glow<M: 'static> {
    base: Base<M>,
    widget: GlowWidget,
    children: Vec<Element<M>>,
}

builder!(Glow<M> => GlowWidget, children);

impl<M: 'static> Glow<M> {
    /// The ring's colour; the accent by default.
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    /// The ring's width and how far its glow spreads, in points.
    pub fn ring(mut self, width: f32, blur: f32) -> Self {
        self.widget.width = width.max(0.0);
        self.widget.blur = blur.max(0.0);
        self
    }

    /// Breathe once every `seconds`; 0 keeps it steady.
    pub fn breathe(mut self, seconds: f32) -> Self {
        self.widget.period = seconds.max(0.0);
        self
    }
}

pub fn glow<M: 'static>(on: bool) -> Glow<M> {
    Glow { base: Base::new(), widget: GlowWidget { on, color: Tone::Accent.into(), width: 1.5, blur: 8.0, period: 0.0 }, children: Vec::new() }
}
