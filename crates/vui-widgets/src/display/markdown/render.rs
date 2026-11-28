//! Markdown blocks laid out as elements: headings, paragraphs, lists, quotes, code, rules, tables
//! and pictures.

use super::*;

pub(crate) type ImageLookup = Rc<dyn Fn(&str) -> Option<ImageId>>;

/// The paragraphs of one build of a document, which type in one at a time: the document they share,
/// and the bytes of text that types in, in all and handed out so far.
struct Typing {
    doc: u64,
    total: usize,
    done: Cell<usize>,
}

impl Typing {
    fn new(blocks: &[Block]) -> Typing {
        let total = blocks
            .iter()
            .map(|b| match b {
                Block::Heading(_, i) | Block::Paragraph(i) | Block::Quote(i) => join(i).len(),
                Block::List { start, items } => items.iter().enumerate().map(|(n, i)| mark(*start, n).len() + join(i).len()).sum(),
                _ => 0,
            })
            .sum();
        Typing { doc: next_document(), total, done: Cell::new(0) }
    }

    /// The bytes after the next `len` bytes, which are handed out now.
    fn after(&self, len: usize) -> usize {
        self.done.set(self.done.get() + len);
        self.total.saturating_sub(self.done.get())
    }
}

/// The mark of a list's `n`th item: its number in a numbered list, else a bullet.
fn mark(start: Option<u64>, n: usize) -> String {
    match start {
        Some(s) => format!("{}.", s + n as u64),
        None => "•".into(),
    }
}

/// How a document is drawn: its text colour and size, where links go, and how pictures are found.
pub struct Markdown<M: 'static> {
    pub(crate) text: String,
    pub(crate) color: Fill,
    pub(crate) role: TextRole,
    pub(crate) on_link: Option<LinkHandler<M>>,
    pub(crate) image: Option<ImageLookup>,
    pub(crate) reveal: bool,
    pub(crate) base: Div<M>,
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

    /// For a document that is still streaming in: its text types itself in at a steady pace as it
    /// arrives, one paragraph, heading or list item at a time in reading order, with its newest words
    /// fading in, and blocks that are not text (code, tables, pictures) fade in when they show up.
    /// What its end has begun shows as it will look once finished, never as markup (see
    /// [`parse_streaming`]). What was already there when it was built without this shows whole, so
    /// turning it off when the stream ends lets the last words finish typing.
    pub fn reveal(mut self, on: bool) -> Self {
        self.reveal = on;
        self
    }

    /// Styles the box that holds the document.
    pub fn with(mut self, f: impl FnOnce(Div<M>) -> Div<M>) -> Self {
        self.base = f(self.base);
        self
    }

    /// Text that types in, in its turn.
    fn para(&self, inlines: Vec<Inline>, typing: &Typing) -> RichText<M> {
        let after = typing.after(join(&inlines).len());
        let mut r = self.cell(inlines).reveal(self.reveal);
        r.widget.doc = typing.doc;
        r.widget.after = after;
        r
    }

    /// Text that shows whole: a table's cells, whose table fades in.
    fn cell(&self, inlines: Vec<Inline>) -> RichText<M> {
        let mut r = rich_inlines(inlines).text_role(self.role).color(self.color);
        r.widget.on_link = self.on_link.clone();
        r
    }

    fn block(&self, i: usize, block: Block, typing: &Typing) -> Element<M> {
        match block {
            Block::Heading(level, text) => {
                let role = if level <= 1 { TextRole::Title } else { self.role };
                let mut r = self.para(text, typing).text_role(role).weight(600);
                if level > 1 {
                    r = r.color(Tone::Text);
                }
                r.margin([if i == 0 { 0.0 } else { 8.0 }, 0.0, 0.0, 0.0]).into_element()
            }
            Block::Paragraph(text) => self.para(text, typing).into_element(),
            Block::Quote(text) => row()
                .gap(12.0)
                .children((div().width(2.0).align_self(Align::Stretch).rounded(Radius::Px(1.0)).bg(Tone::Line), self.para(text, typing).color(Tone::Text2).grow(1.0).min_width(0.0)))
                .into_element(),
            // Each item's mark types in with it, so an item waiting its turn shows nothing.
            Block::List { start, items } => column()
                .gap(4.0)
                .collapse_empty()
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(n, item)| {
                            let mark = self.para(vec![Inline::Text(mark(start, n))], typing).color(Tone::Text3).min_width(16.0);
                            row().gap(8.0).align_items(Align::Start).children((mark, self.para(item, typing).grow(1.0).min_width(0.0)))
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
                        let mut p = self.cell(cell).grow(1.0).basis(0.0).min_width(0.0);
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
        // Streaming text shows what its end has begun as it will look, never as markup.
        let blocks = if self.reveal { parse_streaming(&self.text) } else { parse(&self.text) };
        let typing = Typing::new(&blocks);
        let children: Vec<Element<M>> = blocks
            .into_iter()
            .enumerate()
            .map(|(i, b)| {
                let typed = matches!(b, Block::Paragraph(_) | Block::Heading(..) | Block::Quote(_) | Block::List { .. });
                let element = self.block(i, b, &typing);
                // Text types itself in; anything else fades in as a whole when it first shows up. The
                // box is there either way, so the stream ending does not rebuild what is in it.
                if typed {
                    element
                } else {
                    let wrap = div().min_width(0.0);
                    let wrap = if self.reveal { wrap.appear() } else { wrap };
                    wrap.child(element).into_element()
                }
            })
            .collect();
        let Markdown { base, .. } = self;
        base.children(children).into_element()
    }
}

/// A document of markdown: `markdown(reply).on_link(Msg::Open)`.
pub fn markdown<M: 'static>(text: &str) -> Markdown<M> {
    Markdown { text: text.to_string(), color: Tone::Text.into(), role: TextRole::Body, on_link: None, image: None, reveal: false, base: column().gap(10.0).collapse_empty().min_width(0.0) }
}
