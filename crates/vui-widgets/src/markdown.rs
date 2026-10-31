//! Markdown, drawn with the theme: headings, paragraphs with bold, italic, code and links, lists,
//! quotes, code blocks, rules, tables and pictures. Written for what models write; the parser is
//! small and forgiving, and anything it does not understand shows as plain text, never as markup.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, FontFamily, ImageId, PointerButton, Size, Span, TextRole, TextStyle, Theme, Tone, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessInfo, Align, Base, Builder, Dirty, Div, Element, Env, EventCx, IntoElement, PaintCx, Parent, Radius, Role, Styled, Widget,
    WidgetEvent, builder, column, div, img, label, row,
};

// ---- Parsing -----------------------------------------------------------------------------------

/// A run of inline text, before it meets a theme.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(String),
    Emphasis(String),
    StrongEmphasis(String),
    Code(String),
    Link { text: String, url: String },
}

/// A block of a document.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    /// Items, each its inline text; `start` is the first number of a numbered list.
    List { start: Option<u64>, items: Vec<Vec<Inline>> },
    Quote(Vec<Inline>),
    Code { lang: String, text: String },
    Rule,
    Image { alt: String, src: String },
    Table { head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some((hashes as u8, line[hashes..].trim().trim_end_matches('#').trim_end()))
    } else {
        None
    }
}

fn bullet(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["- ", "* ", "+ ", "• "].iter().find_map(|m| t.strip_prefix(m))
}

fn numbered(line: &str) -> Option<(u64, &str)> {
    let t = line.trim_start();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = &t[digits..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    Some((t[..digits].parse().ok()?, rest))
}

fn rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

fn image(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let src = rest.strip_suffix(')')?;
    (!src.contains(' ') || src.starts_with('<')).then(|| (alt.to_string(), src.trim_matches(['<', '>']).to_string()))
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

fn table_divider(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Splits `text` into blocks.
pub fn parse(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(inlines(&para.join(" "))));
            para.clear();
        }
    };
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut blocks);
            i += 1;
            continue;
        }
        if let Some(fence) = ["```", "~~~"].into_iter().find(|f| t.starts_with(f)) {
            flush(&mut para, &mut blocks);
            let lang = t[3..].trim().to_string();
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                body.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code { lang, text: body.join("\n") });
            i += 1;
            continue;
        }
        if let Some((level, text)) = heading(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Heading(level, inlines(text)));
            i += 1;
            continue;
        }
        if rule(t) && para.is_empty() {
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some((alt, src)) = image(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Image { alt, src });
            i += 1;
            continue;
        }
        if t.starts_with('|') && i + 1 < lines.len() && table_divider(lines[i + 1]) {
            flush(&mut para, &mut blocks);
            let head = cells(t).iter().map(|c| inlines(c)).collect();
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                rows.push(cells(lines[i]).iter().map(|c| inlines(c)).collect());
                i += 1;
            }
            blocks.push(Block::Table { head, rows });
            continue;
        }
        if t.starts_with('>') {
            flush(&mut para, &mut blocks);
            let mut quote = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            blocks.push(Block::Quote(inlines(&quote.join(" "))));
            continue;
        }
        if bullet(line).is_some() || numbered(line).is_some() {
            flush(&mut para, &mut blocks);
            let start = numbered(line).map(|(n, _)| n);
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                let item = if start.is_some() { numbered(l).map(|(_, r)| r) } else { bullet(l) };
                if let Some(item) = item {
                    items.push(item.trim().to_string());
                } else if !l.trim().is_empty() && l.starts_with([' ', '\t']) && !items.is_empty() {
                    // A wrapped or nested line joins the item above it.
                    let last = items.last_mut().expect("checked");
                    let more = bullet(l).or(numbered(l).map(|(_, r)| r)).unwrap_or(l.trim());
                    last.push_str(if bullet(l).is_some() || numbered(l).is_some() { " · " } else { " " });
                    last.push_str(more.trim());
                } else {
                    break;
                }
                i += 1;
            }
            blocks.push(Block::List { start, items: items.iter().map(|s| inlines(s)).collect() });
            continue;
        }
        para.push(t);
        i += 1;
    }
    flush(&mut para, &mut blocks);
    blocks
}

/// Splits a line into styled runs.
pub fn inlines(text: &str) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut plain = String::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut k = 0;
    let push_plain = |plain: &mut String, out: &mut Vec<Inline>| {
        if !plain.is_empty() {
            out.push(Inline::Text(std::mem::take(plain)));
        }
    };
    let word = |c: char| c.is_alphanumeric();
    while k < chars.len() {
        let (at, c) = chars[k];
        let rest = &text[at..];
        // `code`
        if c == '`' {
            let ticks = rest.bytes().take_while(|b| *b == b'`').count();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence) {
                push_plain(&mut plain, &mut out);
                out.push(Inline::Code(rest[ticks..ticks + end].trim().to_string()));
                let next = at + ticks + end + ticks;
                k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                continue;
            }
        }
        // [text](url), and ![alt](src) inline, which shows as its alt text linked to the picture.
        if c == '[' || (c == '!' && rest[1..].starts_with('[')) {
            let open = if c == '!' { 2 } else { 1 };
            if let Some(close) = rest.find("](")
                && close > open - 1
                && let Some(end) = rest[close + 2..].find(')')
            {
                let label = &rest[open..close];
                let url = rest[close + 2..close + 2 + end].trim();
                if !label.contains('\n') && !url.contains(' ') && !url.is_empty() {
                    push_plain(&mut plain, &mut out);
                    out.push(Inline::Link { text: if label.is_empty() { url.to_string() } else { strip(label) }, url: url.to_string() });
                    let next = at + close + 2 + end + 1;
                    k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                    continue;
                }
            }
        }
        // ***both***, **strong** / __strong__, *emphasis* / _emphasis_
        if c == '*' || c == '_' {
            let run = rest.chars().take_while(|x| *x == c).count().min(3);
            let marker = &rest[..run];
            let before = if k == 0 { None } else { Some(chars[k - 1].1) };
            let after = rest[run..].chars().next();
            // `_` inside a word (snake_case) is not markup.
            let opens = after.is_some_and(|a| !a.is_whitespace()) && !(c == '_' && before.is_some_and(word));
            if opens && let Some(end) = rest[run..].find(marker) {
                let inner = &rest[run..run + end];
                let closes = !inner.is_empty() && !inner.ends_with(char::is_whitespace) && !(c == '_' && rest[run + end + run..].chars().next().is_some_and(word));
                if closes {
                    push_plain(&mut plain, &mut out);
                    out.push(match run {
                        3 => Inline::StrongEmphasis(inner.to_string()),
                        2 => Inline::Strong(inner.to_string()),
                        _ => Inline::Emphasis(inner.to_string()),
                    });
                    let next = at + run + end + run;
                    k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                    continue;
                }
            }
        }
        // ~~struck~~ shows as plain text.
        if c == '~' && rest.starts_with("~~") && let Some(end) = rest[2..].find("~~") {
            plain.push_str(&rest[2..2 + end]);
            let next = at + 2 + end + 2;
            k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
            continue;
        }
        // A backslash keeps the next character literal.
        if c == '\\' && k + 1 < chars.len() && chars[k + 1].1.is_ascii_punctuation() {
            plain.push(chars[k + 1].1);
            k += 2;
            continue;
        }
        plain.push(c);
        k += 1;
    }
    push_plain(&mut plain, &mut out);
    out
}

/// The text of some inline markdown with the markup taken out, for places that show plain text.
pub fn strip(text: &str) -> String {
    inlines(text)
        .into_iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) | Inline::Code(t) => t,
            Inline::Link { text, .. } => text,
        })
        .collect()
}

/// The whole document as plain text: what a notch line or a spoken reply should use.
pub fn plain(text: &str) -> String {
    parse(text)
        .into_iter()
        .map(|b| match b {
            Block::Heading(_, i) | Block::Paragraph(i) | Block::Quote(i) => join(&i),
            Block::List { items, .. } => items.iter().map(|i| join(i)).collect::<Vec<_>>().join("; "),
            Block::Code { text, .. } => text,
            Block::Rule => String::new(),
            Block::Image { alt, .. } => alt,
            Block::Table { head, rows } => std::iter::once(&head).chain(rows.iter()).map(|r| r.iter().map(|c| join(c)).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("; "),
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn join(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) | Inline::Code(t) => t.as_str(),
            Inline::Link { text, .. } => text.as_str(),
        })
        .collect()
}

// ---- Rich text ---------------------------------------------------------------------------------

type LinkHandler<M> = Rc<dyn Fn(String) -> M>;

/// The widget behind [`RichText`]: a paragraph of styled spans whose links can be clicked.
pub struct RichTextWidget<M> {
    inlines: Vec<Inline>,
    role: TextRole,
    weight: Option<u16>,
    mono: bool,
    color: Fill,
    on_link: Option<LinkHandler<M>>,
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
    baseline: Option<f32>,
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
    base: Base<M>,
    widget: RichTextWidget<M>,
}

builder!(RichText<M> => RichTextWidget<M>);

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

fn rich_inlines<M: 'static>(inlines: Vec<Inline>) -> RichText<M> {
    RichText { base: Base::new(), widget: RichTextWidget { inlines, role: TextRole::Body, weight: None, mono: false, color: Tone::Text.into(), on_link: None } }
}

// ---- Documents ---------------------------------------------------------------------------------

type ImageLookup = Rc<dyn Fn(&str) -> Option<ImageId>>;

/// How a document is drawn: its text colour and size, where links go, and how pictures are found.
pub struct Markdown<M: 'static> {
    text: String,
    color: Fill,
    role: TextRole,
    on_link: Option<LinkHandler<M>>,
    image: Option<ImageLookup>,
    base: Div<M>,
}

impl<M: 'static> Markdown<M> {
    pub fn color(mut self, fill: impl Into<Fill>) -> Self {
        self.color = fill.into();
        self
    }

    /// Body text in another of the theme's styles, such as `TextRole::Small`.
    pub fn text_role(mut self, role: TextRole) -> Self {
        self.role = role;
        self
    }

    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_link = Some(Rc::new(f));
        self
    }

    /// Finds the picture for an image's address; pictures it cannot find show as their alt text.
    pub fn images(mut self, f: impl Fn(&str) -> Option<ImageId> + 'static) -> Self {
        self.image = Some(Rc::new(f));
        self
    }

    /// Styles the box that holds the document.
    pub fn with(mut self, f: impl FnOnce(Div<M>) -> Div<M>) -> Self {
        self.base = f(self.base);
        self
    }

    fn para(&self, inlines: Vec<Inline>) -> RichText<M> {
        let mut r = rich_inlines(inlines).text_role(self.role).color(self.color);
        r.widget.on_link = self.on_link.clone();
        r
    }

    fn block(&self, i: usize, block: Block) -> Element<M> {
        match block {
            Block::Heading(level, text) => {
                let role = if level <= 1 { TextRole::Title } else { self.role };
                let mut r = self.para(text).text_role(role).weight(600);
                if level > 1 {
                    r = r.color(Tone::Text);
                }
                r.margin([if i == 0 { 0.0 } else { 8.0 }, 0.0, 0.0, 0.0]).into_element()
            }
            Block::Paragraph(text) => self.para(text).into_element(),
            Block::Quote(text) => row()
                .gap(12.0)
                .children((div().width(2.0).align_self(Align::Stretch).rounded(Radius::Px(1.0)).bg(Tone::Line), self.para(text).color(Tone::Text2).grow(1.0).min_width(0.0)))
                .into_element(),
            Block::List { start, items } => column()
                .gap(4.0)
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(n, item)| {
                            let mark = match start {
                                Some(s) => format!("{}.", s + n as u64),
                                None => "•".into(),
                            };
                            row().gap(8.0).align_items(Align::Start).children((
                                label(mark).text_role(self.role).color(Tone::Text3).min_width(16.0),
                                self.para(item).grow(1.0).min_width(0.0),
                            ))
                        })
                        .collect::<Vec<_>>(),
                )
                .into_element(),
            Block::Code { text, .. } => column()
                .padding_y(12.0)
                .padding_x(16.0)
                .rounded(Radius::Card)
                .bg(Tone::Wash)
                .child(label(text).mono().text_role(TextRole::Small).color(Tone::Text))
                .into_element(),
            Block::Rule => div().height(1.0).bg(Tone::Line).margin_y(4.0).into_element(),
            Block::Image { alt, src } => match self.image.as_ref().and_then(|find| find(&src)) {
                Some(id) => img(id).max_width(vui_runtime::pct(100.0)).max_height(360.0).rounded(Radius::Card).name(alt).into_element(),
                None => label(if alt.is_empty() { src } else { alt }).text_role(TextRole::Small).color(Tone::Text3).into_element(),
            },
            Block::Table { head, rows } => {
                let columns = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
                let line = |cells: Vec<Vec<Inline>>, header: bool| {
                    let mut r = row().gap(16.0).padding_y(6.0);
                    for n in 0..columns {
                        let cell = cells.get(n).cloned().unwrap_or_default();
                        let mut p = self.para(cell).grow(1.0).basis(0.0).min_width(0.0);
                        if header {
                            p = p.weight(600);
                        }
                        r = r.child(p);
                    }
                    r
                };
                let mut table = column().child(line(head, true)).child(div().height(1.0).bg(Tone::Line));
                for r in rows {
                    table = table.child(line(r, false));
                }
                table.into_element()
            }
        }
    }
}


impl<M: 'static> IntoElement<M> for Markdown<M> {
    fn into_element(self) -> Element<M> {
        let blocks = parse(&self.text);
        let children: Vec<Element<M>> = blocks.into_iter().enumerate().map(|(i, b)| self.block(i, b)).collect();
        let Markdown { base, .. } = self;
        base.children(children).into_element()
    }
}

/// A document of markdown: `markdown(reply).on_link(Msg::Open)`.
pub fn markdown<M: 'static>(text: &str) -> Markdown<M> {
    Markdown { text: text.to_string(), color: Tone::Text.into(), role: TextRole::Body, on_link: None, image: None, base: column().gap(10.0).min_width(0.0) }
}

