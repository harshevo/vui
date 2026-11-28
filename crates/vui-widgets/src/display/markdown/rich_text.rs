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
    /// The text types itself in when it first shows up, and keeps pace as more is added.
    pub(crate) reveal: bool,
    /// The document it is part of, if any (see [`next_document`]), and how many bytes of text that
    /// types in come after it there.
    pub(crate) doc: u64,
    pub(crate) after: usize,
}

thread_local! {
    /// Documents handed out so far.
    static DOCUMENTS: Cell<u64> = const { Cell::new(0) };
    /// The document with a paragraph typing this frame, the frame (its time), and that paragraph.
    static TYPING: Cell<(u64, u32, usize)> = const { Cell::new((0, 0, 0)) };
}

/// A new document for paragraphs that type in one at a time: of all its paragraphs still typing,
/// only the first does, so text that arrives in a burst types in reading order rather than in
/// several places at once. Each build of a document takes a new one; all that matters is that its
/// paragraphs share it.
pub(crate) fn next_document() -> u64 {
    DOCUMENTS.with(|d| {
        d.set(d.get() + 1);
        d.get()
    })
}

/// Bytes a second the text types in at, however little is left.
const REVEAL_MIN_RATE: f32 = 90.0;
/// Seconds behind the text the reveal runs: what has arrived is typed in over about this long, so
/// text that comes in bursts reads as a steady flow.
const REVEAL_LAG: f32 = 0.3;
/// Points over which newly typed text fades in, and the steps the fade is drawn in.
const REVEAL_FADE: f32 = 28.0;
const REVEAL_BANDS: usize = 7;

impl<M> RichTextWidget<M> {
    fn len(&self) -> usize {
        self.inlines
            .iter()
            .map(|i| match i {
                Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) | Inline::Code(t) => t.len(),
                Inline::Link { text, .. } => text.len(),
            })
            .sum()
    }

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
    /// Bytes typed in so far, and how fast the reveal is going, in bytes a second.
    pub(crate) shown: f32,
    pub(crate) rate: f32,
    /// The width the text was last wrapped to, and the line the reveal was on then.
    pub(crate) width: Option<f32>,
    pub(crate) line: usize,
}

impl RichTextState {
    fn revealing(&self, len: usize) -> bool {
        self.shown < len as f32
    }
}

/// The visual line holding byte `at`: the first that ends after it, else the last.
fn line_of(lines: &[vui_core::VisualLine], at: f32) -> usize {
    lines.iter().position(|l| (l.end as f32) > at).unwrap_or(lines.len().saturating_sub(1))
}

/// Where byte `at` (fractional) sits along the text, as if every line were laid end to end: the
/// widths of the lines before its own, plus how far into its line it is.
fn distance(layout: &vui_core::TextLayout, lines: &[vui_core::VisualLine], at: f32) -> f32 {
    let n = line_of(lines, at);
    let before: f32 = lines[..n].iter().map(|l| l.width).sum();
    let line = lines[n];
    let text = layout.text();
    let byte = (at.floor() as usize).clamp(line.start, line.end);
    let mut lo = byte.min(text.len());
    while lo > 0 && !text.is_char_boundary(lo) {
        lo -= 1;
    }
    let x0 = if lo >= line.end { line.width } else { layout.caret_rect(lo).x.min(line.width) };
    let mut hi = (lo + 1).min(text.len());
    while hi < text.len() && !text.is_char_boundary(hi) {
        hi += 1;
    }
    let x1 = if hi >= line.end { line.width } else { layout.caret_rect(hi).x.min(line.width) };
    let frac = if hi > lo { ((at - lo as f32) / (hi - lo) as f32).clamp(0.0, 1.0) } else { 0.0 };
    before + x0 + (x1 - x0).max(0.0) * frac
}

impl<M: 'static> Widget<M> for RichTextWidget<M> {
    type State = RichTextState;

    fn create_state(&self) -> RichTextState {
        // Text that is there from the start shows whole; only a reveal types in from nothing.
        let shown = if self.reveal { 0.0 } else { f32::INFINITY };
        RichTextState { shown, ..RichTextState::default() }
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
        let mut h = layout.size().h;
        if state.revealing(self.len()) && state.shown <= 0.0 {
            // Waiting its turn to type in, it takes no room until it starts.
            state.width = width.or(state.width);
            state.line = 0;
            return size(known.x.unwrap_or(layout.size().w), 0.0);
        }
        if let (Some(w), true) = (width, state.revealing(self.len())) {
            // While it types in, the text is only as tall as the lines typed so far, so it grows a
            // line at a time as the reveal reaches each one.
            state.width = Some(w);
            let lines = layout.visual_lines();
            if !lines.is_empty() {
                state.line = line_of(&lines, state.shown);
                let l = lines[state.line];
                h = (l.top + l.height).min(h);
            }
        }
        size(known.x.unwrap_or(layout.size().w), h)
    }

    fn baseline(&self, state: &RichTextState) -> Option<f32> {
        state.baseline
    }

    fn paint(&self, state: &RichTextState, cx: &mut PaintCx) {
        let theme = cx.theme().clone();
        let layout = cx.fonts().layout_rich_cached(&self.spans(&theme), &self.style(&theme), Some(cx.exact.w + 0.01));
        if !state.revealing(self.len()) {
            cx.text(layout, cx.content.origin(), self.color);
            return;
        }
        paint_reveal(cx, layout, self.color, state.shown);
    }

    fn animating(&self, state: &RichTextState) -> bool {
        state.revealing(self.len())
    }

    fn tick(&self, state: &mut RichTextState, cx: &mut TickCx, dt: f32) -> bool {
        let len = self.len() as f32;
        if state.shown >= len {
            return false;
        }
        if cx.still() {
            state.shown = len;
            cx.request_layout();
            return false;
        }
        // One paragraph of a document types at a time: the first still typing that ticks this frame.
        if self.doc != 0 {
            let me = state as *const RichTextState as usize;
            let frame = cx.time.to_bits();
            let (doc, time, who) = TYPING.with(Cell::get);
            if doc == self.doc && time == frame && who != me {
                return true;
            }
            TYPING.with(|t| t.set((self.doc, frame, me)));
        }
        // The pace follows what is waiting, here and in the document after it, eased so a burst
        // speeds it up smoothly rather than at once; a paragraph taking over starts at that pace.
        let want = ((len - state.shown + self.after as f32) / REVEAL_LAG).max(REVEAL_MIN_RATE);
        let starting = state.shown <= 0.0;
        if state.rate <= 0.0 {
            state.rate = want;
        }
        state.rate += (want - state.rate) * (dt * 10.0).min(1.0);
        state.shown = (state.shown + state.rate.max(REVEAL_MIN_RATE) * dt).min(len);
        cx.request_paint();
        if starting {
            cx.request_layout();
        }
        // Reaching a new line makes the text a line taller.
        let line = match state.width {
            Some(w) => {
                let theme = cx.env.theme.clone();
                let layout = cx.env.fonts().layout_rich_cached(&self.spans(&theme), &self.style(&theme), Some(w));
                line_of(&layout.visual_lines(), state.shown)
            }
            None => 0,
        };
        if line != state.line || state.shown >= len {
            cx.request_layout();
        }
        state.shown < len
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

/// Draws `layout` typed in up to byte `shown`: what is behind the last few points of the reveal
/// whole, those points fading from whole to nothing, and the rest not at all. Each part is the text
/// drawn again under its own clip, so it works with any renderer.
fn paint_reveal(cx: &mut PaintCx, layout: Rc<vui_core::TextLayout>, color: Fill, shown: f32) {
    let lines = layout.visual_lines();
    if lines.is_empty() {
        return;
    }
    let origin = cx.content.origin();
    let front = distance(&layout, &lines, shown);
    let full = front - REVEAL_FADE;
    let outer = cx.clip;
    let scale = cx.env.scale.max(1.0);
    // Clip edges on device pixels, so neighbouring bands neither overlap nor leave a gap.
    let snap = |x: f32| (x * scale).round() / scale;
    let draw = |cx: &mut PaintCx, x0: f32, x1: f32, top: f32, bottom: f32, alpha: f32| {
        let (x0, x1) = (snap(origin.x + x0), snap(origin.x + x1));
        if x1 <= x0 || alpha <= 0.0 {
            return;
        }
        // A hair inside the snapped edges, so rounding outwards lands back on them.
        let band = vui_core::Rect { x: x0 + 0.01 / scale, y: origin.y + top, w: x1 - x0 - 0.02 / scale, h: bottom - top };
        let clip = match outer {
            Some(c) => match c.intersection(band) {
                Some(r) => r,
                None => return,
            },
            None => band,
        };
        let saved = cx.alpha;
        cx.clip = Some(clip);
        cx.alpha = saved * alpha;
        cx.text(layout.clone(), origin, color);
        cx.alpha = saved;
    };
    let mut at = 0.0;
    let mut whole_to: Option<f32> = None;
    let wide = cx.exact.w + REVEAL_FADE;
    for l in &lines {
        let (start, end) = (at, at + l.width);
        at = end;
        let (top, bottom) = (l.top, l.top + l.height);
        if end <= full {
            // A line typed in completely joins the block drawn whole above.
            whole_to = Some(bottom);
            continue;
        }
        if start >= front {
            break;
        }
        // Part of this line is whole, part fading, and the rest still to come.
        if full > start {
            draw(cx, -REVEAL_FADE, full - start, top, bottom, 1.0);
        }
        let step = REVEAL_FADE / REVEAL_BANDS as f32;
        for b in 0..REVEAL_BANDS {
            let (d0, d1) = (full + step * b as f32, full + step * (b + 1) as f32);
            if d1 <= start || d0 >= end {
                continue;
            }
            // The band at either end of the line reaches past it, for glyphs that overhang.
            let x0 = if d0 <= start { -REVEAL_FADE } else { d0 - start };
            let x1 = if d1 >= end { wide } else { d1 - start };
            let alpha = 1.0 - (b as f32 + 0.5) / REVEAL_BANDS as f32;
            draw(cx, x0, x1, top, bottom, alpha);
        }
    }
    if let Some(bottom) = whole_to {
        draw(cx, -REVEAL_FADE, wide, 0.0, bottom, 1.0);
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

    /// Types the text in when it first shows up, and keeps a steady pace as more is added, for text
    /// that streams in. Text already there when the paragraph is built without it shows whole.
    pub fn reveal(mut self, on: bool) -> Self {
        self.widget.reveal = on;
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
    RichText { base: Base::new(), widget: RichTextWidget { inlines, role: TextRole::Body, weight: None, mono: false, color: Tone::Text.into(), on_link: None, reveal: false, doc: 0, after: 0 } }
}

builder!(RichText<M> => RichTextWidget<M>);
