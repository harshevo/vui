//! Rich text: inline runs drawn as one wrapped paragraph, with links that can be clicked.

use super::*;

pub(crate) type LinkHandler<M> = Rc<dyn Fn(String) -> M>;

/// The widget behind [`RichText`]: a paragraph of styled spans whose links can be clicked.
pub struct RichTextWidget<M> {
    pub(crate) inlines: Vec<Inline>,
    pub(crate) role: TextRole,
    pub(crate) weight: Option<u16>,
    pub(crate) mono: bool,
    pub(crate) color: Fill,
    pub(crate) on_link: Option<LinkHandler<M>>,
}

impl<M> RichTextWidget<M> {
    fn style(&self, theme: &Theme) -> TextStyle {
        let mut style = TextStyle::from_token(theme.types.get(self.role));
        if let Some(w) = self.weight {
            style.weight = w;
        }
        if self.mono {
            style.family = FontFamily::Mono;
        }
        style
    }

    fn spans(&self, theme: &Theme) -> Vec<Span> {
        let palette = &theme.palette;
        self.inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) => Span::new(t.clone()),
                Inline::Strong(t) => Span::new(t.clone()).weight(600),
                Inline::Emphasis(t) => Span::new(t.clone()).italic(),
                Inline::StrongEmphasis(t) => Span::new(t.clone()).weight(600).italic(),
                Inline::Code(t) => Span::new(t.clone()).mono().color(palette.text_2),
                Inline::Link { text, .. } => Span::new(text.clone()).color(palette.accent),
            })
            .collect()
    }

    fn link_at(&self, cx: &EventCx<M>, theme: &Theme, pos: vui_core::Point) -> Option<String> {
        let layout = cx.env.fonts().layout_rich_cached(&self.spans(theme), &self.style(theme), Some(cx.content.w + 0.01));
        let local = vui_core::Point { x: pos.x - cx.content.x, y: pos.y - cx.content.y };
        if local.y > layout.size().h || local.x > layout.size().w {
            return None;
        }
        let index = layout.hit(local);
        match self.inlines.get(layout.span_at(index)?)? {
            Inline::Link { url, .. } => Some(url.clone()),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct RichTextState {
    pub(crate) baseline: Option<f32>,
}

impl<M: 'static> Widget<M> for RichTextWidget<M> {
    type State = RichTextState;

    fn create_state(&self) -> RichTextState {
        RichTextState::default()
    }

    fn update(&self, old: &RichTextWidget<M>, _state: &mut RichTextState) -> Dirty {
        if self.inlines != old.inlines || self.role != old.role || self.weight != old.weight || self.mono != old.mono {
            Dirty::Layout
        } else if self.color != old.color {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut RichTextState, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let theme = &env.theme;
        let style = self.style(theme);
        let spans = self.spans(theme);
        let fonts = env.fonts();
        let width = match (known.x, avail.x) {
            (Some(w), _) | (None, Avail::Definite(w)) => Some(w),
            (None, Avail::MinContent) => Some(fonts.min_content_width(&join(&self.inlines), &style)),
            (None, Avail::MaxContent) => None,
        };
        let layout = fonts.layout_rich_cached(&spans, &style, width);
        state.baseline = Some(layout.baseline());
        size(known.x.unwrap_or(layout.size().w), layout.size().h)
    }

    fn baseline(&self, state: &RichTextState) -> Option<f32> {
        state.baseline
    }

    fn paint(&self, _state: &RichTextState, cx: &mut PaintCx) {
        let theme = cx.theme().clone();
        let layout = cx.fonts().layout_rich_cached(&self.spans(&theme), &self.style(&theme), Some(cx.exact.w + 0.01));
        cx.text(layout, cx.content.origin(), self.color);
    }

    fn event(&self, _state: &mut RichTextState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let Some(on_link) = &self.on_link else { return false };
        let theme = cx.env.theme.clone();
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) if self.link_at(cx, &theme, *pos).is_some() => {
                cx.capture_pointer();
                true
            }
            WidgetEvent::Input(Event::PointerUp { pos, button: PointerButton::Primary, .. }) => {
                cx.release_pointer();
                match self.link_at(cx, &theme, *pos) {
                    Some(url) => {
                        cx.emit(on_link(url));
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }

    fn cursor(&self) -> Option<CursorIcon> {
        self.inlines.iter().any(|i| matches!(i, Inline::Link { .. })).then_some(CursorIcon::Pointer)
    }

    fn access(&self, _state: &RichTextState, info: &mut AccessInfo) {
        info.role = Some(Role::Label);
        info.label = Some(join(&self.inlines));
    }
}

/// A paragraph of inline markdown: `rich_text("Run **cargo test** in `vui/`")`.
pub struct RichText<M: 'static> {
    pub(crate) base: Base<M>,
    pub(crate) widget: RichTextWidget<M>,
}

impl<M: 'static> RichText<M> {
    pub fn text_role(mut self, role: TextRole) -> Self {
        self.widget.role = role;
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.widget.weight = Some(weight);
        self
    }

    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.color = fill.into();
        self
    }

    /// What to send when a link is clicked: its address.
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.widget.on_link = Some(Rc::new(f));
        self
    }
}

pub fn rich_text<M: 'static>(text: &str) -> RichText<M> {
    rich_inlines(inlines(text))
}

pub(crate) fn rich_inlines<M: 'static>(inlines: Vec<Inline>) -> RichText<M> {
    RichText { base: Base::new(), widget: RichTextWidget { inlines, role: TextRole::Body, weight: None, mono: false, color: Tone::Text.into(), on_link: None } }
}

builder!(RichText<M> => RichTextWidget<M>);
