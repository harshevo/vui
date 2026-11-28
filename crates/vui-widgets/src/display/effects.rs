//! Boxes that light up what is inside them: a shimmer sweeping across work in progress and a glow
//! ring around something selected or listening. Both fade in and out, and use the renderer's
//! built-in effects pass, so they need nothing from the app.

use vui_core::{Fill, Mode, Quad, Shadow, Spring, SpringValue, Tone, effects};
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
        cx.custom(effects::shimmer(cx.rect, radius, color, self.period, cx.effect_time()));
    }

    fn tick(&self, state: &mut EffectState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = cx.step(&mut state.shown, dt);
        (self.on && !cx.still()) || moving
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
        cx.custom(effects::glow(cx.rect, radius, color, self.width, self.blur, self.period, cx.effect_time()));
    }

    fn tick(&self, state: &mut EffectState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = cx.step(&mut state.shown, dt);
        (self.on && self.period > 0.0 && !cx.still()) || moving
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

/// How much of a full lift the pointer alone gives a surface it is over.
const HOVER_LIFT: f32 = 0.4;

/// The widget behind [`Lift`].
pub struct LiftWidget {
    on: bool,
    rest: Fill,
    raised: Fill,
    shadow: Shadow,
    glow: Option<(Fill, f32, f32)>,
    ring: Option<Fill>,
}

impl<M: 'static> Widget<M> for LiftWidget {
    type State = EffectState;

    fn create_state(&self) -> EffectState {
        EffectState::new(self.on)
    }

    fn update(&self, old: &LiftWidget, state: &mut EffectState) -> Dirty {
        state.shown.set_target(if self.on { 1.0 } else { 0.0 });
        let same = self.rest == old.rest && self.raised == old.raised && self.shadow == old.shadow && self.glow == old.glow && self.ring == old.ring;
        if same { Dirty::None } else { Dirty::Paint }
    }

    fn paint(&self, state: &EffectState, cx: &mut PaintCx) {
        let shown = state.shown.value().clamp(0.0, 1.0);
        // The pointer lifts a surface part of the way, whether or not it is the one in focus.
        let level = (shown + (1.0 - shown) * HOVER_LIFT * cx.hover.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        let radius = cx.radii.tl.max(cx.radii.br);
        if level > 0.01 {
            // A dark surface takes a stronger shadow than a light one, as `Builder::shadow` gives it.
            let k = if cx.env.theme.mode == Mode::Dark { 2.5 } else { 1.0 };
            cx.custom(effects::shadow(cx.rect, radius, self.shadow.times(k * level)));
        }
        let (rest, raised) = (self.rest.resolve(cx.palette()), self.raised.resolve(cx.palette()));
        let surface = rest.lerp(raised, level);
        if surface.a > 0.0 {
            cx.fill_rect(cx.rect, Fill::Solid(surface));
        }
        if let Some((fill, width, blur)) = self.glow
            && shown > 0.01
        {
            let color = fill.resolve(cx.palette()).mul_alpha(shown);
            cx.custom(effects::glow(cx.rect, radius, color, width, blur, 0.0, cx.effect_time()));
        }
    }

    // The ring goes over what sits on the surface, so a full-bleed child never hides it.
    fn paint_over(&self, state: &EffectState, cx: &mut PaintCx) {
        let width = cx.env.theme.focus_border;
        let shown = state.shown.value().clamp(0.0, 1.0);
        if let Some(fill) = self.ring
            && width > 0.0
            && shown > 0.01
        {
            let color = fill.resolve(cx.palette()).mul_alpha(shown * cx.alpha);
            cx.quad(Quad::new(cx.rect).radii(cx.radii).border(width, color));
        }
    }

    fn tick(&self, state: &mut EffectState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.shown, dt)
    }

    fn animating(&self, state: &EffectState) -> bool {
        !state.shown.is_settled()
    }
}

/// A surface that rises off the page while `on`: its colour moves from `rest` to `raised`, and its
/// shadow and an optional soft glow grow in, all with a spring, so a card in focus lifts and the
/// others sink back into the page. The pointer lifts it part of the way. What an element's own
/// `.bg` and `.shadow` do cannot animate; this does, and it takes no layout transition, so resizing
/// the card never lags. Its children sit on the surface.
///
/// ```
/// use vui_core::{Fill, Tone};
/// use vui_runtime::{Builder, Parent, Radius, Styled, column};
/// use vui_widgets::lift;
/// let card = lift::<()>(true)
///     .rounded(Radius::Card)
///     .surface(Fill::Tone(Tone::Bg, 0.5), Tone::Bg)
///     .glow(Fill::Tone(Tone::Accent, 0.3), 1.0, 12.0)
///     .child(column::<()>());
/// ```
pub struct Lift<M: 'static> {
    base: Base<M>,
    widget: LiftWidget,
    children: Vec<Element<M>>,
}

builder!(Lift<M> => LiftWidget, children);

impl<M: 'static> Lift<M> {
    /// The surface's colour at rest and when raised. None by default: only the shadow and glow draw.
    pub fn surface(mut self, rest: impl Into<Fill>, raised: impl Into<Fill>) -> Self {
        self.widget.rest = rest.into();
        self.widget.raised = raised.into();
        self
    }

    /// The shadow at full lift; [`Shadow::RAISED`] by default.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.widget.shadow = shadow;
        self
    }

    /// A soft glow of `fill` around the box at full lift: a ring `width` points wide that spreads
    /// `blur` points outward.
    pub fn glow(mut self, fill: impl Into<Fill>, width: f32, blur: f32) -> Self {
        self.widget.glow = Some((fill.into(), width.max(0.0), blur.max(0.0)));
        self
    }

    /// A crisp line of `fill` just inside the box while raised, as wide as the theme's
    /// `focus_border` (none at 0). It comes in with the lift and has no soft edge.
    pub fn ring(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.ring = Some(fill.into());
        self
    }
}

pub fn lift<M: 'static>(on: bool) -> Lift<M> {
    Lift { base: Base::new(), widget: LiftWidget { on, rest: Fill::NONE, raised: Fill::NONE, shadow: Shadow::RAISED, glow: None, ring: None }, children: Vec::new() }
}
