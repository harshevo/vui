//! The fonts: the bundled faces and the system's, shaping text into layouts, and the cache that
//! keeps layouts between frames.

use super::*;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct CacheKey {
    pub(crate) text: String,
    pub(crate) family: FontFamily,
    pub(crate) size: u32,
    pub(crate) line_height: u32,
    pub(crate) weight: u16,
    pub(crate) align: TextAlign,
    pub(crate) wrap: TextWrap,
    pub(crate) max_lines: Option<usize>,
    pub(crate) spacing: u32,
    pub(crate) width: Option<u32>,
}

impl CacheKey {
    fn new(text: &str, style: &TextStyle, max_width: Option<f32>) -> CacheKey {
        CacheKey {
            text: text.to_owned(),
            family: style.family.clone(),
            size: style.size.to_bits(),
            line_height: style.line_height.to_bits(),
            weight: style.weight,
            align: style.align,
            wrap: style.wrap,
            max_lines: style.max_lines,
            spacing: style.letter_spacing.to_bits(),
            // 1/64 px is finer than any layout difference that matters.
            width: max_width.map(|w| (w * 64.0).round() as u32),
        }
    }
}

#[derive(Default)]
pub(crate) struct TextCache {
    pub(crate) map: HashMap<CacheKey, (Rc<TextLayout>, u64)>,
    pub(crate) frame: u64,
}

pub(crate) struct Shared {
    pub(crate) system: RefCell<FontSystem>,
    pub(crate) cache: RefCell<TextCache>,
    pub(crate) sans: String,
    pub(crate) mono: String,
}

/// The font database, shaping engine and layout cache. Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct Fonts {
    pub(crate) shared: Rc<Shared>,
}

impl Fonts {
    /// The bundled fonts plus every installed font, so scripts Inter lacks (CJK, Arabic, emoji) still
    /// render through fallback.
    pub fn new() -> Fonts {
        Fonts::build(true)
    }

    /// Only the bundled fonts. Deterministic across machines, which golden-image tests need.
    pub fn bundled_only() -> Fonts {
        Fonts::build(false)
    }

    fn build(system_fonts: bool) -> Fonts {
        let mut db = fontdb::Database::new();
        if system_fonts {
            db.load_system_fonts();
        }
        for bytes in vui_fonts::all() {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("Inter");
        db.set_monospace_family("JetBrains Mono");
        let system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
        Fonts {
            shared: Rc::new(Shared {
                system: RefCell::new(system),
                cache: RefCell::new(TextCache::default()),
                sans: "Inter".to_string(),
                mono: "JetBrains Mono".to_string(),
            }),
        }
    }

    /// Exclusive access to the shaping engine, for the renderer.
    pub fn system(&self) -> RefMut<'_, FontSystem> {
        self.shared.system.borrow_mut()
    }

    fn family_name<'a>(&'a self, f: &'a FontFamily) -> &'a str {
        match f {
            FontFamily::Sans => &self.shared.sans,
            FontFamily::Mono => &self.shared.mono,
            FontFamily::Named(n) => n,
        }
    }

    /// Shapes `text` without caching. `max_width` is the wrap width; `None` means one unbroken line
    /// per paragraph.
    pub fn layout(&self, text: &str, style: &TextStyle, max_width: Option<f32>) -> TextLayout {
        match (style.max_lines, max_width) {
            (Some(n), Some(w)) if n >= 2 => self.layout_truncated(text, style, w, n),
            _ => self.layout_raw(text, style, max_width),
        }
    }

    /// Several lines with an ellipsis: cut at a word boundary rather than mid-word, which is what a
    /// reader expects and what cosmic-text's own ellipsis does not do.
    fn layout_truncated(&self, text: &str, style: &TextStyle, width: f32, lines: usize) -> TextLayout {
        let mut plain = style.clone();
        plain.max_lines = None;
        let full = self.layout_raw(text, &plain, Some(width));
        if full.lines <= lines {
            return full;
        }
        let mut head = text[..full.end_of_visual_line(lines - 1)].trim_end().to_string();
        loop {
            let candidate = format!("{head}\u{2026}");
            let layout = self.layout_raw(&candidate, &plain, Some(width));
            if layout.lines <= lines || head.is_empty() {
                return layout;
            }
            match head.rfind(char::is_whitespace) {
                Some(i) => head.truncate(i),
                None => {
                    head.pop();
                }
            }
            head.truncate(head.trim_end().len());
        }
    }

    fn layout_raw(&self, text: &str, style: &TextStyle, max_width: Option<f32>) -> TextLayout {
        self.shape(text, &[], style, max_width)
    }

    /// Shapes spans of rich text as one paragraph flow, wrapped at `max_width`.
    pub fn layout_rich(&self, spans: &[Span], style: &TextStyle, max_width: Option<f32>) -> TextLayout {
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        self.shape(&text, spans, style, max_width)
    }

    /// Like [`Fonts::layout_rich`], reusing an earlier frame's result when nothing changed.
    pub fn layout_rich_cached(&self, spans: &[Span], style: &TextStyle, max_width: Option<f32>) -> Rc<TextLayout> {
        let key = CacheKey::new(&format!("\u{1}{spans:?}"), style, max_width);
        {
            let mut cache = self.shared.cache.borrow_mut();
            let frame = cache.frame;
            if let Some((layout, used)) = cache.map.get_mut(&key) {
                *used = frame;
                return layout.clone();
            }
        }
        let layout = Rc::new(self.layout_rich(spans, style, max_width));
        let mut cache = self.shared.cache.borrow_mut();
        let frame = cache.frame;
        cache.map.insert(key, (layout.clone(), frame));
        layout
    }

    fn shape(&self, text: &str, spans: &[Span], style: &TextStyle, max_width: Option<f32>) -> TextLayout {
        let mut fs = self.shared.system.borrow_mut();
        let mut buffer = Buffer::new(&mut fs, Metrics::new(style.size, style.line_height));
        // A box sized to its text is often handed that width back after padding is added and taken
        // away again, a rounding error narrower; without a hair of slack its last word would wrap.
        buffer.set_size(max_width.map(|w| w + WIDTH_SLACK), None);
        buffer.set_wrap(match style.wrap {
            TextWrap::Words => Wrap::WordOrGlyph,
            TextWrap::WordsOnly => Wrap::Word,
            TextWrap::None => Wrap::None,
        });
        if let Some(n) = style.max_lines {
            buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(n)));
        }
        let attrs = Attrs::new()
            .family(Family::Name(self.family_name(&style.family)))
            .weight(Weight(style.weight))
            .letter_spacing(style.letter_spacing);
        let align = match style.align {
            TextAlign::Start => None,
            TextAlign::Center => Some(Align::Center),
            TextAlign::End => Some(Align::End),
        };
        let mut span_starts = Vec::new();
        if spans.is_empty() {
            buffer.set_text(text, &attrs, Shaping::Advanced, align);
        } else {
            let mono: &str = &self.shared.mono;
            let mut at = 0;
            let styled: Vec<(&str, Attrs)> = spans
                .iter()
                .enumerate()
                .map(|(i, span)| {
                    span_starts.push(at);
                    at += span.text.len();
                    let mut a = attrs.clone().metadata(i);
                    if let Some(w) = span.weight {
                        a = a.weight(Weight(w));
                    }
                    if span.italic {
                        a = a.style(cosmic_text::Style::Italic);
                    }
                    if span.mono {
                        a = a.family(Family::Name(mono));
                    }
                    if let Some(c) = span.color {
                        let [r, g, b, alpha] = c.to_rgba8();
                        a = a.color(cosmic_text::Color::rgba(r, g, b, alpha));
                    }
                    (span.text.as_str(), a)
                })
                .collect();
            buffer.set_rich_text(styled, &attrs, Shaping::Advanced, align);
        }
        buffer.shape_until_scroll(&mut fs, false);

        let (mut w, mut bottom, mut lines) = (0.0f32, 0.0f32, 0usize);
        for run in buffer.layout_runs() {
            w = w.max(run.line_w);
            bottom = bottom.max(run.line_top + run.line_height);
            lines += 1;
        }
        if lines == 0 {
            bottom = style.line_height;
            lines = 1;
        }

        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        TextLayout { buffer, size: size(w, bottom), lines, text: Arc::from(text), line_starts, line_height: style.line_height, span_starts }
    }

    /// Like [`Fonts::layout`], reusing the result from an earlier frame when nothing changed.
    pub fn layout_cached(&self, text: &str, style: &TextStyle, max_width: Option<f32>) -> Rc<TextLayout> {
        let key = CacheKey::new(text, style, max_width);
        {
            let mut cache = self.shared.cache.borrow_mut();
            let frame = cache.frame;
            if let Some((layout, used)) = cache.map.get_mut(&key) {
                *used = frame;
                return layout.clone();
            }
        }
        let layout = Rc::new(self.layout(text, style, max_width));
        let mut cache = self.shared.cache.borrow_mut();
        let frame = cache.frame;
        cache.map.insert(key, (layout.clone(), frame));
        layout
    }

    pub fn measure(&self, text: &str, style: &TextStyle, max_width: Option<f32>) -> Size {
        self.layout_cached(text, style, max_width).size()
    }

    /// The narrowest the text can be wrapped without cutting a word: the width of its longest
    /// unbreakable run. Layout uses it as the least a line of text can shrink to.
    pub fn min_content_width(&self, text: &str, style: &TextStyle) -> f32 {
        let mut narrow = style.clone();
        narrow.wrap = TextWrap::WordsOnly;
        narrow.max_lines = None;
        self.layout_cached(text, &narrow, Some(0.0)).size().w
    }

    /// Call once per frame: drops layouts nobody has asked for in the last two frames.
    pub fn end_frame(&self) {
        let mut cache = self.shared.cache.borrow_mut();
        cache.frame += 1;
        let frame = cache.frame;
        cache.map.retain(|_, (_, used)| frame - *used <= 2);
    }

    pub fn cached_layouts(&self) -> usize {
        self.shared.cache.borrow().map.len()
    }
}

impl Default for Fonts {
    fn default() -> Fonts {
        Fonts::new()
    }
}
