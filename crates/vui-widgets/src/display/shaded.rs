//! Boxes drawn by the renderer's shading pass: an aurora of slowly drifting colour behind the
//! contents, a beam of light running round the edge, and a spotlight that follows the pointer. Like
//! the glow and the shimmer they need nothing from the app, and draw the same on the CPU renderer.

use vui_core::shading::{self, Ramp};
use vui_core::{Color, Event, Fill, Palette, Point, Spring, SpringValue, Tone, pt};
use vui_runtime::{Base, Dirty, Element, EventCx, PaintCx, TickCx, Widget, WidgetEvent, builder};

use super::effects::EffectState;

fn ramp(fills: &[Fill], palette: &Palette, alpha: f32) -> Ramp {
    let colors: Vec<Color> = fills.iter().map(|f| f.resolve(palette).mul_alpha(alpha)).collect();
    Ramp::even(&colors)
}

/// The widget behind [`Aurora`].
pub struct AuroraWidget {
    colors: Vec<Fill>,
    period: f32,
    blob: f32,
    grain: f32,
    still: Option<f32>,
}

impl<M: 'static> Widget<M> for AuroraWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &AuroraWidget, _state: &mut ()) -> Dirty {
        let same = self.colors == old.colors
            && self.period == old.period
            && self.blob == old.blob
            && self.grain == old.grain
            && self.still == old.still;
        if same { Dirty::None } else { Dirty::Paint }
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        let ramp = ramp(&self.colors, &cx.env.theme.palette, 1.0);
        let radius = cx.radii.tl.max(cx.radii.br);
        let time = self.still.unwrap_or(cx.effect_time());
        cx.custom(shading::aurora(cx.rect, radius, &ramp, self.period, self.blob, self.grain, time));
    }

    fn tick(&self, _state: &mut (), cx: &mut TickCx, _dt: f32) -> bool {
        self.still.is_none() && !cx.still()
    }

    fn animating(&self, _state: &()) -> bool {
        self.still.is_none()
    }
}

/// A box filled with soft colour drifting behind its contents, for heroes, empty states and
/// onboarding: `aurora().rounded(16.0).padding(32.0).child(...)`.
pub struct Aurora<M: 'static> {
    base: Base<M>,
    widget: AuroraWidget,
    children: Vec<Element<M>>,
}

builder!(Aurora<M> => AuroraWidget, children);

impl<M: 'static> Aurora<M> {
    /// Up to four colours; the first is the ground the others drift over.
    pub fn colors<F: Into<Fill>>(mut self, colors: impl IntoIterator<Item = F>) -> Self {
        self.widget.colors = colors.into_iter().map(Into::into).take(shading::MAX_STOPS).collect();
        self
    }

    /// Seconds for the colours to come round again; 12 by default.
    pub fn period(mut self, seconds: f32) -> Self {
        self.widget.period = seconds.max(0.5);
        self
    }

    /// How big each patch of colour is, as a share of the longer side; 0.45 by default.
    pub fn blob(mut self, size: f32) -> Self {
        self.widget.blob = size.clamp(0.05, 2.0);
        self
    }

    /// Film grain over the colour, which hides banding; 0.03 by default, 0 for none.
    pub fn grain(mut self, amount: f32) -> Self {
        self.widget.grain = amount.clamp(0.0, 0.5);
        self
    }

    /// Holds the colours where they are at `seconds` instead of moving them: for screenshots, tests
    /// and people who asked for less motion.
    pub fn still(mut self, seconds: f32) -> Self {
        self.widget.still = Some(seconds);
        self
    }
}

pub fn aurora<M: 'static>() -> Aurora<M> {
    // Tinted rather than saturated, so text in either theme stays readable over it.
    let colors = vec![Tone::Bg.into(), Fill::Tone(Tone::Accent, 0.55), Fill::Tone(Tone::Purple, 0.45), Fill::Tone(Tone::Blue, 0.4)];
    Aurora {
        base: Base::new(),
        widget: AuroraWidget { colors, period: 12.0, blob: 0.45, grain: 0.03, still: None },
        children: Vec::new(),
    }
}

/// The widget behind [`BorderBeam`].
pub struct BeamWidget {
    on: bool,
    colors: Vec<Fill>,
    width: f32,
    period: f32,
    length: f32,
    still: Option<f32>,
}

impl BeamWidget {
    fn moving(&self) -> bool {
        self.on && self.still.is_none()
    }
}

impl<M: 'static> Widget<M> for BeamWidget {
    type State = EffectState;

    fn create_state(&self) -> EffectState {
        EffectState::new(self.on)
    }

    fn update(&self, old: &BeamWidget, state: &mut EffectState) -> Dirty {
        state.shown.set_target(if self.on { 1.0 } else { 0.0 });
        let same = self.colors == old.colors
            && self.width == old.width
            && self.period == old.period
            && self.length == old.length
            && self.still == old.still;
        if same { Dirty::None } else { Dirty::Paint }
    }

    fn paint_over(&self, state: &EffectState, cx: &mut PaintCx) {
        let shown = state.shown.value().clamp(0.0, 1.0);
        if shown <= 0.01 {
            return;
        }
        let ramp = ramp(&self.colors, &cx.env.theme.palette, shown);
        let radius = cx.radii.tl.max(cx.radii.br);
        let time = self.still.unwrap_or(cx.effect_time());
        cx.custom(shading::beam(cx.rect, radius, self.width, &ramp, self.period, self.length, time));
    }

    fn tick(&self, state: &mut EffectState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = cx.step(&mut state.shown, dt);
        (self.moving() && !cx.still()) || moving
    }

    fn animating(&self, state: &EffectState) -> bool {
        self.moving() || !state.shown.is_settled()
    }
}

/// A box with a streak of light running round its edge while `on`, for something working, live or
/// new: `border_beam(syncing).rounded(12.0).child(...)`.
pub struct BorderBeam<M: 'static> {
    base: Base<M>,
    widget: BeamWidget,
    children: Vec<Element<M>>,
}

builder!(BorderBeam<M> => BeamWidget, children);

impl<M: 'static> BorderBeam<M> {
    /// The streak's colours from its tail to its head, up to four; the tail usually transparent.
    pub fn colors<F: Into<Fill>>(mut self, colors: impl IntoIterator<Item = F>) -> Self {
        self.widget.colors = colors.into_iter().map(Into::into).take(shading::MAX_STOPS).collect();
        self
    }

    /// The streak's width in points; 1.5 by default.
    pub fn width(mut self, points: f32) -> Self {
        self.widget.width = points.max(0.5);
        self
    }

    /// Seconds per lap; 4 by default.
    pub fn period(mut self, seconds: f32) -> Self {
        self.widget.period = seconds.max(0.2);
        self
    }

    /// How much of the edge the streak covers, from 0 to 1; 0.3 by default.
    pub fn length(mut self, share: f32) -> Self {
        self.widget.length = share.clamp(0.02, 1.0);
        self
    }

    /// Holds the streak where it is at `seconds`.
    pub fn still(mut self, seconds: f32) -> Self {
        self.widget.still = Some(seconds);
        self
    }
}

pub fn border_beam<M: 'static>(on: bool) -> BorderBeam<M> {
    let colors = vec![Fill::from(Color::TRANSPARENT), Tone::Accent.into(), Tone::Purple.into()];
    BorderBeam {
        base: Base::new(),
        widget: BeamWidget { on, colors, width: 1.5, period: 4.0, length: 0.3, still: None },
        children: Vec::new(),
    }
}

/// The widget behind [`Spotlight`].
pub struct SpotlightWidget {
    color: Fill,
    reach: f32,
    at: Option<Point>,
}

/// Where the pointer is over a [`Spotlight`], from its top left, and how lit it is.
pub struct SpotlightState {
    shown: SpringValue,
    at: Point,
}

impl<M: 'static> Widget<M> for SpotlightWidget {
    type State = SpotlightState;

    fn create_state(&self) -> SpotlightState {
        let lit = if self.at.is_some() { 1.0 } else { 0.0 };
        SpotlightState { shown: SpringValue::new(lit, Spring::SMOOTH), at: self.at.unwrap_or(pt(0.0, 0.0)) }
    }

    fn update(&self, old: &SpotlightWidget, state: &mut SpotlightState) -> Dirty {
        if let Some(at) = self.at {
            state.at = at;
            state.shown.set_target(1.0);
        }
        let same = self.color == old.color && self.reach == old.reach && self.at == old.at;
        if same { Dirty::None } else { Dirty::Paint }
    }

    fn paint(&self, state: &SpotlightState, cx: &mut PaintCx) {
        let shown = state.shown.value().clamp(0.0, 1.0);
        if shown <= 0.01 {
            return;
        }
        let color = self.color.resolve(&cx.env.theme.palette).mul_alpha(shown);
        let radius = cx.radii.tl.max(cx.radii.br);
        cx.custom(shading::spotlight(cx.rect, radius, state.at, self.reach, color));
    }

    fn event(&self, state: &mut SpotlightState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        if self.at.is_some() {
            return false;
        }
        match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                state.at = pt(pos.x - cx.rect.x, pos.y - cx.rect.y);
                state.shown.set_target(1.0);
                cx.request_frame();
                cx.request_paint();
            }
            WidgetEvent::HoverChanged(false) => {
                state.shown.set_target(0.0);
                cx.request_frame();
            }
            _ => {}
        }
        // The pointer still belongs to whatever is under it.
        false
    }

    fn tick(&self, state: &mut SpotlightState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.shown, dt)
    }

    fn animating(&self, state: &SpotlightState) -> bool {
        !state.shown.is_settled()
    }
}

/// A box lit from wherever the pointer is over it, fading when the pointer leaves: cards on a dark
/// page, pricing tiers, feature grids. `spotlight().rounded(12.0).child(...)`.
pub struct Spotlight<M: 'static> {
    base: Base<M>,
    widget: SpotlightWidget,
    children: Vec<Element<M>>,
}

builder!(Spotlight<M> => SpotlightWidget, children);

impl<M: 'static> Spotlight<M> {
    /// The light's colour; a faint accent by default.
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    /// How far the light reaches, in points; 320 by default.
    pub fn reach(mut self, points: f32) -> Self {
        self.widget.reach = points.max(8.0);
        self
    }

    /// Keeps the light at a point, from the box's top left, instead of following the pointer.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.widget.at = Some(pt(x, y));
        self
    }
}

pub fn spotlight<M: 'static>() -> Spotlight<M> {
    Spotlight {
        base: Base::new(),
        widget: SpotlightWidget { color: Fill::Tone(Tone::Accent, 0.25), reach: 320.0, at: None },
        children: Vec::new(),
    }
}
