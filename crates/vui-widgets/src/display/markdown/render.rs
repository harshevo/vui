//! Markdown blocks laid out as elements: headings, paragraphs, lists, quotes, code, rules, tables
//! and pictures.

use super::*;

pub(crate) type ImageLookup = Rc<dyn Fn(&str) -> Option<ImageId>>;

/// How a document is drawn: its text colour and size, where links go, and how pictures are found.
pub struct Markdown<M: 'static> {
    pub(crate) text: String,
    pub(crate) color: Fill,
    pub(crate) role: TextRole,
    pub(crate) on_link: Option<LinkHandler<M>>,
    pub(crate) image: Option<ImageLookup>,
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
