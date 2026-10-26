//! The two leaf widgets everything else is built from: text and images.

use vui_core::{Fill, Fit, FontFamily, ImageCmd, ImageId, Sampling, Size, TextAlign, TextRole, TextStyle, TextWrap, Theme, Tone, size};

use crate::access::{AccessInfo, Role};
use crate::element::Base;
use crate::layout::{Avail, Pair};
use crate::style::{Overflow, Styled};
use crate::widget::{Dirty, Env, PaintCx, Widget};

/// What decides how a label is shaped. Changing any of it changes the label's size.
#[derive(Clone, Debug, PartialEq)]
struct TextSpec {
    text: String,
    role: TextRole,
    size: Option<f32>,
    weight: Option<u16>,
    mono: bool,
    align: TextAlign,
    wrap: bool,
    max_lines: Option<usize>,
}

impl TextSpec {
    fn style(&self, theme: &Theme) -> TextStyle {
        let token = theme.types.get(self.role);
        let mut style = TextStyle::from_token(token);
        if let Some(size) = self.size {
            style.line_height = (size * token.line_height / token.size).round();
            style.size = size;
        }
        if let Some(weight) = self.weight {
            style.weight = weight;
        }
        if self.mono || self.role == TextRole::Mono {
            style.family = FontFamily::Mono;
        }
        style.align = self.align;
        if !self.wrap {
            style.wrap = TextWrap::None;
        }
        style.max_lines = self.max_lines;
        style
    }
}

/// The widget behind [`Label`].
pub struct LabelWidget {
    spec: TextSpec,
    color: Fill,
}

#[derive(Default)]
pub struct LabelState {
    baseline: Option<f32>,
}

impl<M: 'static> Widget<M> for LabelWidget {
    type State = LabelState;

    fn create_state(&self) -> LabelState {
        LabelState::default()
    }

    fn update(&self, old: &LabelWidget, _state: &mut LabelState) -> Dirty {
        if self.spec != old.spec {
            Dirty::Layout
        } else if self.color != old.color {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut LabelState, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let style = self.spec.style(&env.theme);
        let fonts = env.fonts();
        let width = match (known.x, avail.x) {
            (Some(w), _) | (None, Avail::Definite(w)) => Some(w),
            (None, Avail::MinContent) => Some(fonts.min_content_width(&self.spec.text, &style)),
            (None, Avail::MaxContent) => None,
        };
        let layout = fonts.layout_cached(&self.spec.text, &style, width);
        state.baseline = Some(layout.baseline());
        size(known.x.unwrap_or(layout.size().w), layout.size().h)
    }

    fn baseline(&self, state: &LabelState) -> Option<f32> {
        state.baseline
    }

    fn paint(&self, _state: &LabelState, cx: &mut PaintCx) {
        let style = self.spec.style(cx.theme());
        // A hair of slack: a box sized to its text can come out a rounding error narrower than the text.
        let layout = cx.fonts().layout_cached(&self.spec.text, &style, Some(cx.exact.w + 0.01));
        cx.text(layout, cx.content.origin(), self.color);
    }

    fn access(&self, _state: &LabelState, info: &mut AccessInfo) {
        info.role = Some(if self.spec.role == TextRole::Title { Role::Heading } else { Role::Label });
        info.label = Some(self.spec.text.clone());
    }
}

/// Text in one of the theme's styles.
pub struct Label<M: 'static> {
    base: Base<M>,
    widget: LabelWidget,
}

crate::builder!(Label<M> => LabelWidget);

impl<M: 'static> Label<M> {
    fn new(text: impl Into<String>, role: TextRole) -> Label<M> {
        Label {
            base: Base::new(),
            widget: LabelWidget {
                spec: TextSpec {
                    text: text.into(),
                    role,
                    size: None,
                    weight: None,
                    mono: false,
                    align: TextAlign::Start,
                    wrap: true,
                    max_lines: None,
                },
                color: Tone::Text.into(),
            },
        }
    }

    /// Switches to another of the theme's text styles.
    pub fn text_role(mut self, role: TextRole) -> Self {
        self.widget.spec.role = role;
        self
    }

    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.widget.spec.weight = Some(weight);
        self
    }

    /// A size other than the style's; the line height follows in proportion.
    pub fn font_size(mut self, size: f32) -> Self {
        self.widget.spec.size = Some(size);
        self
    }

    pub fn mono(mut self) -> Self {
        self.widget.spec.mono = true;
        self
    }

    pub fn align_text(mut self, align: TextAlign) -> Self {
        self.widget.spec.align = align;
        self
    }

    /// Cuts the text off with an ellipsis after this many lines. The label can then shrink below its
    /// text, so it clips what does not fit.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.widget.spec.max_lines = Some(lines);
        self.style(|s| s.overflow = Overflow::Hidden)
    }

    /// One line, however long.
    pub fn no_wrap(mut self) -> Self {
        self.widget.spec.wrap = false;
        self
    }
}

/// Body text.
pub fn label<M: 'static>(text: impl Into<String>) -> Label<M> {
    Label::new(text, TextRole::Body)
}

/// A heading.
pub fn title<M: 'static>(text: impl Into<String>) -> Label<M> {
    Label::new(text, TextRole::Title)
}

/// Small print: times, counts, hints.
pub fn small<M: 'static>(text: impl Into<String>) -> Label<M> {
    Label::new(text, TextRole::Small)
}

/// What decides how an image is drawn.
#[derive(Clone, Debug, PartialEq)]
struct ImageSpec {
    id: ImageId,
    fit: Fit,
    sampling: Sampling,
    mask: Option<Fill>,
    ring: bool,
}

/// The widget behind [`Img`].
pub struct ImgWidget {
    spec: ImageSpec,
}

impl<M: 'static> Widget<M> for ImgWidget {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, old: &ImgWidget, _state: &mut ()) -> Dirty {
        if self.spec.id != old.spec.id {
            Dirty::Layout
        } else if self.spec != old.spec {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, _state: &mut (), env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let natural = env.images().size(self.spec.id).unwrap_or(Size::ZERO);
        let ratio = if natural.h > 0.0 { natural.w / natural.h } else { 0.0 };
        match (known.x, known.y) {
            (Some(w), Some(h)) => size(w, h),
            (Some(w), None) => size(w, if ratio > 0.0 { w / ratio } else { 0.0 }),
            (None, Some(h)) => size(h * ratio, h),
            (None, None) => match avail.x {
                Avail::Definite(w) if natural.w > w && ratio > 0.0 => size(w, w / ratio),
                _ => natural,
            },
        }
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        let natural = cx.env.images().size(self.spec.id).unwrap_or(Size::ZERO);
        let (dest, uv) = vui_core::fit_image(natural, cx.content, self.spec.fit);
        let mut cmd = ImageCmd::new(self.spec.id, dest).uv(uv).radii(cx.radii).smooth(cx.smooth).sampling(self.spec.sampling);
        let palette = cx.palette().clone();
        if let Some(tint) = self.spec.mask {
            cmd = cmd.mask(tint.resolve(&palette));
        }
        if self.spec.ring {
            cmd = cmd.ring(1.0, Fill::from(Tone::Ring).resolve(&palette));
        }
        cx.image(cmd);
    }

    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::Image);
    }
}

/// A picture from the shared image store, or an icon drawn as a coloured stencil.
pub struct Img<M: 'static> {
    base: Base<M>,
    widget: ImgWidget,
}

crate::builder!(Img<M> => ImgWidget);

impl<M: 'static> Img<M> {
    /// How the picture fills its box.
    pub fn fit(mut self, fit: Fit) -> Self {
        self.widget.spec.fit = fit;
        self
    }

    pub fn sampling(mut self, sampling: Sampling) -> Self {
        self.widget.spec.sampling = sampling;
        self
    }

    /// Paints the image's alpha channel in one colour: how icons follow the theme.
    pub fn tint(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.spec.mask = Some(fill.into());
        self
    }

    /// A hairline just inside the edge, so pale pictures do not bleed into the page.
    pub fn ring(mut self) -> Self {
        self.widget.spec.ring = true;
        self
    }
}

pub fn img<M: 'static>(id: ImageId) -> Img<M> {
    Img {
        base: Base::new(),
        widget: ImgWidget { spec: ImageSpec { id, fit: Fit::Cover, sampling: Sampling::Smooth, mask: None, ring: false } },
    }
}
