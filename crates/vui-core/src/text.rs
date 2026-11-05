//! Text: fonts, shaping, measuring and caret geometry.
//!
//! `Fonts` owns the font database and the shaping engine (cosmic-text). Layout turns a string and a
//! `TextStyle` into a `TextLayout`: shaped glyphs, a size for the layout engine to use, and the
//! geometry text fields need (caret positions, selection rectangles, hit testing). Layouts are cached
//! across frames, so a screenful of unchanged labels costs nothing to re-shape.

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use cosmic_text::{Align, Attrs, Buffer, Cursor, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping, Weight, Wrap, fontdb};

use crate::geom::{Point, Rect, Size, rect, size};
use crate::theme::TextToken;

/// Re-exported so the renderer and backends use the exact same cosmic-text.
pub use cosmic_text;


/// How far past its width a line may run before it wraps: far below a visible pixel, far above
/// the rounding error of adding and removing padding.
const WIDTH_SLACK: f32 = 0.05;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The interface font (Inter by default).
    Sans,
    /// The monospace font (JetBrains Mono by default).
    Mono,
    Named(Arc<str>),
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextWrap {
    /// Break at word boundaries, and inside a word only if it is wider than the line.
    #[default]
    Words,
    /// Break at word boundaries only: a word wider than the line overflows it.
    WordsOnly,
    /// One line, however long.
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub align: TextAlign,
    pub wrap: TextWrap,
    /// Cut off after this many lines with an ellipsis.
    pub max_lines: Option<usize>,
    /// Extra space between letters, in ems.
    pub letter_spacing: f32,
}

impl TextStyle {
    pub fn new(size: f32, line_height: f32) -> TextStyle {
        TextStyle {
            family: FontFamily::Sans,
            size,
            line_height,
            weight: 400,
            align: TextAlign::Start,
            wrap: TextWrap::Words,
            max_lines: None,
            letter_spacing: 0.0,
        }
    }

    pub fn from_token(token: TextToken) -> TextStyle {
        TextStyle::new(token.size, token.line_height).weight(token.weight)
    }

    pub fn weight(mut self, weight: u16) -> TextStyle {
        self.weight = weight;
        self
    }

    pub fn mono(mut self) -> TextStyle {
        self.family = FontFamily::Mono;
        self
    }

    pub fn family(mut self, family: FontFamily) -> TextStyle {
        self.family = family;
        self
    }

    pub fn align(mut self, align: TextAlign) -> TextStyle {
        self.align = align;
        self
    }

    pub fn no_wrap(mut self) -> TextStyle {
        self.wrap = TextWrap::None;
        self
    }

    pub fn max_lines(mut self, lines: usize) -> TextStyle {
        self.max_lines = Some(lines);
        self
    }

    pub fn letter_spacing(mut self, em: f32) -> TextStyle {
        self.letter_spacing = em;
        self
    }
}

/// A run of text styled apart from the rest of its paragraph: bold, italic, code or a link. Anything
/// left `None` follows the paragraph's [`TextStyle`] and colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub weight: Option<u16>,
    pub italic: bool,
    pub mono: bool,
    /// Its own colour; it then ignores the element's opacity.
    pub color: Option<crate::Color>,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Span {
        Span { text: text.into(), ..Span::default() }
    }

    pub fn weight(mut self, weight: u16) -> Span {
        self.weight = Some(weight);
        self
    }

    pub fn italic(mut self) -> Span {
        self.italic = true;
        self
    }

    pub fn mono(mut self) -> Span {
        self.mono = true;
        self
    }

    pub fn color(mut self, color: crate::Color) -> Span {
        self.color = Some(color);
        self
    }
}

/// Shaped text, ready to measure and paint.
pub struct TextLayout {
    buffer: Buffer,
    size: Size,
    lines: usize,
    text: Arc<str>,
    /// Byte offset where each paragraph (split at `\n`) starts.
    line_starts: Vec<usize>,
    line_height: f32,
    /// Byte offset where each span starts, for rich text.
    span_starts: Vec<usize>,
}

impl fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextLayout").field("text", &self.text).field("size", &self.size).field("lines", &self.lines).finish()
    }
}

impl TextLayout {
    /// The space the text occupies. Its width is the widest line, not the wrap width.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Visual lines after wrapping.
    pub fn line_count(&self) -> usize {
        self.lines
    }

    /// Which span the byte `index` is in, for rich text.
    pub fn span_at(&self, index: usize) -> Option<usize> {
        if self.span_starts.is_empty() {
            return None;
        }
        Some(self.span_starts.partition_point(|&start| start <= index).saturating_sub(1))
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Distance from the top of the layout to the first line's baseline.
    pub fn baseline(&self) -> f32 {
        self.buffer.layout_runs().next().map_or(self.line_height * 0.8, |r| r.line_y)
    }

    /// Byte offset in the text where visual line `index` (after wrapping) ends.
    fn end_of_visual_line(&self, index: usize) -> usize {
        let run = self.buffer.layout_runs().nth(index);
        match run {
            Some(run) => {
                let end = run.glyphs.last().map_or(0, |g| g.end);
                self.line_starts[run.line_i] + end
            }
            None => self.text.len(),
        }
    }

    /// Splits a byte index into (paragraph, byte offset inside it).
    fn locate(&self, index: usize) -> (usize, usize) {
        let index = index.min(self.text.len());
        let line = self.line_starts.partition_point(|&s| s <= index).saturating_sub(1);
        (line, index - self.line_starts[line])
    }

    /// The caret for a byte index: a 1 px wide rectangle the height of its line.
    pub fn caret_rect(&self, index: usize) -> Rect {
        let (line_i, idx) = self.locate(index);
        let mut fallback = None;
        for run in self.buffer.layout_runs() {
            if run.line_i != line_i {
                continue;
            }
            for g in run.glyphs {
                if g.start <= idx && idx < g.end {
                    let x = if g.level.is_rtl() { g.x + g.w } else { g.x };
                    return rect(x, run.line_top, 1.0, run.line_height);
                }
            }
            match run.glyphs.last() {
                Some(g) if idx >= g.end => {
                    let x = if g.level.is_rtl() { g.x } else { g.x + g.w };
                    fallback = Some(rect(x, run.line_top, 1.0, run.line_height));
                }
                None => fallback = Some(rect(0.0, run.line_top, 1.0, run.line_height)),
                _ => {}
            }
        }
        fallback.unwrap_or_else(|| rect(0.0, 0.0, 1.0, self.line_height))
    }

    /// Rectangles covering the bytes `start..end`, one per visual line.
    pub fn selection_rects(&self, start: usize, end: usize) -> Vec<Rect> {
        if start >= end {
            return Vec::new();
        }
        let (l0, i0) = self.locate(start);
        let (l1, i1) = self.locate(end);
        let (c0, c1) = (Cursor::new(l0, i0), Cursor::new(l1, i1));
        let mut out = Vec::new();
        for run in self.buffer.layout_runs() {
            for (x, w) in run.highlight(c0, c1) {
                out.push(rect(x, run.line_top, w, run.line_height));
            }
        }
        out
    }

    /// The byte index of the character boundary nearest to `p` (relative to the layout's origin).
    pub fn hit(&self, p: Point) -> usize {
        match self.buffer.hit(p.x, p.y) {
            Some(c) => (self.line_starts.get(c.line).copied().unwrap_or(0) + c.index).min(self.text.len()),
            None if p.y < 0.0 => 0,
            None => self.text.len(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    text: String,
    family: FontFamily,
    size: u32,
    line_height: u32,
    weight: u16,
    align: TextAlign,
    wrap: TextWrap,
    max_lines: Option<usize>,
    spacing: u32,
    width: Option<u32>,
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
struct TextCache {
    map: HashMap<CacheKey, (Rc<TextLayout>, u64)>,
    frame: u64,
}

struct Shared {
    system: RefCell<FontSystem>,
    cache: RefCell<TextCache>,
    sans: String,
    mono: String,
}

/// The font database, shaping engine and layout cache. Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct Fonts {
    shared: Rc<Shared>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pt;

    fn body() -> TextStyle {
        TextStyle::new(14.0, 21.0)
    }

    fn face_family(fonts: &Fonts, layout: &TextLayout) -> (String, u16) {
        let id = layout.buffer().layout_runs().next().unwrap().glyphs[0].font_id;
        let fs = fonts.system();
        let face = fs.db().face(id).unwrap();
        (face.families[0].0.clone(), face.weight.0)
    }

    #[test]
    fn inter_is_the_default_face_and_weights_pick_different_files() {
        let fonts = Fonts::bundled_only();
        let regular = fonts.layout("Hello", &body(), None);
        let medium = fonts.layout("Hello", &body().weight(500), None);
        let semibold = fonts.layout("Hello", &body().weight(600), None);
        let (name, w) = face_family(&fonts, &regular);
        assert!(name.starts_with("Inter"), "{name}");
        assert_eq!(w, 400);
        assert_eq!(face_family(&fonts, &medium).1, 500);
        assert_eq!(face_family(&fonts, &semibold).1, 600);
        assert!(semibold.size().w > regular.size().w, "semibold is wider than regular");
        assert!(medium.size().w > regular.size().w && medium.size().w < semibold.size().w);
    }

    #[test]
    fn mono_uses_jetbrains_mono_with_equal_advances() {
        let fonts = Fonts::bundled_only();
        let a = fonts.layout("iiiiii", &body().mono(), None);
        let b = fonts.layout("WWWWWW", &body().mono(), None);
        assert!(face_family(&fonts, &a).0.contains("JetBrains"), "{:?}", face_family(&fonts, &a));
        assert!((a.size().w - b.size().w).abs() < 0.01);
        let prop_a = fonts.layout("iiiiii", &body(), None);
        let prop_b = fonts.layout("WWWWWW", &body(), None);
        assert!(prop_b.size().w > prop_a.size().w * 2.0, "the sans font is proportional");
    }

    #[test]
    fn height_follows_line_height_and_wrapping_adds_lines() {
        let fonts = Fonts::bundled_only();
        let one = fonts.layout("Hello world", &body(), Some(400.0));
        assert_eq!(one.line_count(), 1);
        assert_eq!(one.size().h, 21.0);
        let wrapped = fonts.layout("Hello world, this sentence is long enough to wrap onto lines", &body(), Some(120.0));
        assert!(wrapped.line_count() >= 3, "{}", wrapped.line_count());
        assert!(wrapped.size().w <= 120.0 + 0.5, "{}", wrapped.size().w);
        assert_eq!(wrapped.size().h, 21.0 * wrapped.line_count() as f32);
    }

    #[test]
    fn no_wrap_keeps_one_line() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("a very long single line of text that would otherwise wrap", &body().no_wrap(), Some(60.0));
        assert_eq!(l.line_count(), 1);
        assert!(l.size().w > 60.0);
    }

    #[test]
    fn min_content_is_the_longest_unbreakable_word() {
        let fonts = Fonts::bundled_only();
        let one_line = |s: &str| fonts.layout(s, &body().no_wrap(), None).size().w;
        let min = fonts.min_content_width("a wonderful little world", &body());
        assert!((min - one_line("wonderful")).abs() < 0.5, "{min} vs {}", one_line("wonderful"));
        assert_eq!(fonts.min_content_width("", &body()), 0.0);
        // ellipsis limits do not make the text narrower than its words
        let capped = fonts.min_content_width("a wonderful little world", &body().max_lines(1));
        assert_eq!(capped, min);
    }

    #[test]
    fn words_only_wrapping_never_cuts_a_word() {
        let fonts = Fonts::bundled_only();
        let mut style = body();
        style.wrap = TextWrap::WordsOnly;
        let long = "abcdefghijklmnopqrstuvwxyz";
        let l = fonts.layout(long, &style, Some(40.0));
        assert_eq!(l.line_count(), 1);
        assert!(l.size().w > 40.0, "the word overflows instead of breaking: {}", l.size().w);
        // the default still breaks a word that cannot fit
        assert!(fonts.layout(long, &body(), Some(40.0)).line_count() > 1);
    }

    #[test]
    fn newlines_start_new_paragraphs() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("one\ntwo\nthree", &body(), None);
        assert_eq!(l.line_count(), 3);
        assert_eq!(l.size().h, 63.0);
    }

    #[test]
    fn max_lines_ellipsizes() {
        let fonts = Fonts::bundled_only();
        let text = "The quick brown fox jumps over the lazy dog and keeps running across the field";
        let full = fonts.layout(text, &body(), Some(140.0));
        let cut = fonts.layout(text, &body().max_lines(2), Some(140.0));
        assert!(full.line_count() > 2);
        assert_eq!(cut.line_count(), 2);
        assert!(cut.size().w <= 140.5);
    }

    #[test]
    fn multi_line_truncation_cuts_at_a_word_and_ends_with_an_ellipsis() {
        let fonts = Fonts::bundled_only();
        let text = "Each permission row ticks when macOS reports the grant, and Continue stays grey until the required ones are done.";
        let cut = fonts.layout(text, &body().max_lines(3), Some(220.0));
        assert_eq!(cut.line_count(), 3);
        let shown = cut.text();
        assert!(shown.ends_with('\u{2026}'), "{shown}");
        let kept = shown.trim_end_matches('\u{2026}');
        assert!(text.starts_with(kept), "the kept text is a prefix of the original: {kept:?}");
        assert!(text[kept.len()..].starts_with(' '), "the cut falls on a word boundary: {kept:?}");
        // text that already fits is left alone
        let short = fonts.layout("Short text", &body().max_lines(3), Some(220.0));
        assert_eq!(short.text(), "Short text");
    }

    #[test]
    fn single_line_truncation_uses_the_ellipsis_too() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("Invoice-0423-final-revised-copy.docx", &body().max_lines(1), Some(120.0));
        assert_eq!(l.line_count(), 1);
        assert!(l.size().w <= 120.5, "{}", l.size().w);
    }

    #[test]
    fn empty_text_still_has_a_line() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("", &body(), Some(100.0));
        assert_eq!(l.size().h, 21.0);
        assert_eq!(l.caret_rect(0).h, 21.0);
        assert_eq!(l.hit(pt(50.0, 5.0)), 0);
    }

    #[test]
    fn caret_moves_right_with_the_index_and_ends_at_the_text_width() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("Hello world", &body(), None);
        let mut last = -1.0;
        for i in 0..=l.text().len() {
            let x = l.caret_rect(i).x;
            assert!(x >= last, "caret went backwards at {i}: {x} < {last}");
            last = x;
        }
        assert_eq!(l.caret_rect(0).x, 0.0);
        assert!((l.caret_rect(11).x - l.size().w).abs() < 0.5);
    }

    #[test]
    fn hit_testing_inverts_caret_positions() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("Hello world", &body(), None);
        for i in [0, 1, 4, 5, 6, 10, 11] {
            let c = l.caret_rect(i);
            assert_eq!(l.hit(pt(c.x + 0.5, c.y + 5.0)), i, "index {i}");
        }
        assert_eq!(l.hit(pt(-20.0, 5.0)), 0);
        assert_eq!(l.hit(pt(1000.0, 5.0)), 11);
    }

    #[test]
    fn selection_covers_the_selected_glyphs() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("Hello world", &body(), None);
        let rects = l.selection_rects(0, 5);
        assert_eq!(rects.len(), 1);
        assert!((rects[0].x - 0.0).abs() < 0.5);
        assert!((rects[0].right() - l.caret_rect(5).x).abs() < 0.5);
        assert!(l.selection_rects(3, 3).is_empty());
        // a selection across a wrap produces one rect per visual line
        let w = fonts.layout("alpha beta gamma delta epsilon", &body(), Some(70.0));
        assert!(w.line_count() >= 3);
        assert!(w.selection_rects(0, w.text().len()).len() >= 3);
    }

    #[test]
    fn caret_after_a_newline_is_on_the_next_line() {
        let fonts = Fonts::bundled_only();
        let l = fonts.layout("ab\ncd", &body(), None);
        assert_eq!(l.caret_rect(2).y, 0.0);
        assert_eq!(l.caret_rect(3).y, 21.0);
        assert_eq!(l.caret_rect(3).x, 0.0);
    }

    #[test]
    fn centered_text_is_offset_inside_the_wrap_width() {
        let fonts = Fonts::bundled_only();
        let left = fonts.layout("Hi", &body(), Some(200.0));
        let centered = fonts.layout("Hi", &body().align(TextAlign::Center), Some(200.0));
        assert!(centered.caret_rect(0).x > 80.0, "{}", centered.caret_rect(0).x);
        assert_eq!(left.caret_rect(0).x, 0.0);
    }

    #[test]
    fn the_cache_returns_the_same_layout_and_evicts_stale_ones() {
        let fonts = Fonts::bundled_only();
        let a = fonts.layout_cached("cached", &body(), Some(100.0));
        let b = fonts.layout_cached("cached", &body(), Some(100.0));
        assert!(Rc::ptr_eq(&a, &b));
        let c = fonts.layout_cached("cached", &body(), Some(200.0));
        assert!(!Rc::ptr_eq(&a, &c), "a different wrap width is a different layout");
        assert_eq!(fonts.cached_layouts(), 2);
        for _ in 0..4 {
            fonts.end_frame();
        }
        assert_eq!(fonts.cached_layouts(), 0);
    }

    #[test]
    fn the_title_token_is_bigger_and_heavier() {
        let t = crate::theme::Theme::light().types;
        let fonts = Fonts::bundled_only();
        let title = fonts.layout("Allow three things", &TextStyle::from_token(t.title), None);
        let body_l = fonts.layout("Allow three things", &TextStyle::from_token(t.body), None);
        assert!(title.size().w > body_l.size().w * 1.5);
        assert_eq!(title.size().h, 32.0);
    }
}
