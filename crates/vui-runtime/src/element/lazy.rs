//! Virtualised lists: a scrolling column of any number of rows of one height, of which only the ones
//! in view (and a few either side) are built, laid out and painted.

use std::ops::Range;

use super::*;
use crate::style::{Len, Overflow};

/// Rows built on demand: how many, how tall, and how to build one.
pub(crate) struct Lazy<M: 'static> {
    pub count: usize,
    pub extent: f32,
    pub overscan: usize,
    pub build: Rc<dyn Fn(usize) -> Element<M>>,
}

impl<M: 'static> Lazy<M> {
    pub fn map<N: 'static>(self, f: &Rc<dyn Fn(M) -> N>) -> Lazy<N> {
        let (build, f) = (self.build, f.clone());
        Lazy { count: self.count, extent: self.extent, overscan: self.overscan, build: Rc::new(move |i| build(i).map_rc(&f)) }
    }

    /// The rows to have built for a view `height` tall scrolled `offset` down, or `None` while
    /// `shown` still covers what is in view.
    pub fn wanted(&self, shown: &Range<usize>, offset: f32, height: f32) -> Option<Range<usize>> {
        if height <= 0.0 {
            return None;
        }
        let first = ((offset / self.extent).floor().max(0.0) as usize).min(self.count);
        let last = (((offset + height) / self.extent).ceil().max(0.0) as usize).min(self.count);
        if shown.start <= first && last <= shown.end && shown.end <= self.count {
            return None;
        }
        Some(first.saturating_sub(self.overscan)..(last + self.overscan).min(self.count))
    }

    /// The rows before the first one is laid out: as many as a tall window shows.
    pub fn first(&self) -> Range<usize> {
        0..self.count.min(((1200.0 / self.extent).ceil() as usize).max(1) + self.overscan)
    }

    /// The children for `range`: a spacer for the rows above, the rows, and a spacer for the rows
    /// below, so the content is as tall as every row together.
    pub fn children(&self, range: Range<usize>) -> Vec<Element<M>> {
        let spacer = |key: &'static str, rows: usize| {
            let mut base = Base::new();
            base.style.height = Len::Px(rows as f32 * self.extent);
            base.style.shrink = 0.0;
            base.key = Some(Key::from(key));
            Element::new(Container, base, Vec::new())
        };
        let mut out = Vec::with_capacity(range.len() + 2);
        out.push(spacer("vui.lazy.above", range.start));
        for i in range.clone() {
            let mut row = (self.build)(i);
            row.base.style.height = Len::Px(self.extent);
            row.base.style.shrink = 0.0;
            if row.base.key.is_none() {
                row.base.key = Some(Key::from(i));
            }
            out.push(row);
        }
        out.push(spacer("vui.lazy.below", self.count - range.end));
        out
    }
}

/// A scrolling list that builds only the rows in view: see [`virtual_list`].
pub struct VirtualList<M: 'static> {
    base: Base<M>,
    lazy: Lazy<M>,
}

impl<M: 'static> VirtualList<M> {
    /// How many rows past each end of the view are kept built, so a quick scroll does not show
    /// empty space for a frame; 4 by default.
    pub fn overscan(mut self, rows: usize) -> Self {
        self.lazy.overscan = rows;
        self
    }
}

impl<M: 'static> Styled for VirtualList<M> {
    fn style_mut(&mut self) -> &mut Style {
        self.base.style_mut()
    }
}

impl<M: 'static> Builder<M> for VirtualList<M> {
    fn base_mut(&mut self) -> &mut Base<M> {
        &mut self.base
    }
}

impl<M: 'static> IntoElement<M> for VirtualList<M> {
    fn into_element(self) -> Element<M> {
        let mut el = Element::new(Container, self.base, Vec::new());
        el.lazy = Some(self.lazy);
        el
    }
}

impl<M: 'static> IntoChildren<M> for VirtualList<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}

/// A scrolling column of `count` rows, each `row_height` tall, of which only the ones in view are
/// built: `build(i)` is called for row `i` when it scrolls into view, and again whenever the view is
/// rebuilt while it is there. A list of a hundred thousand rows costs what the visible twenty do.
///
/// ```
/// use vui_runtime::{Builder, Styled, label, virtual_list};
///
/// let names: Vec<String> = (0..100_000).map(|i| format!("Contact {i}")).collect();
/// let list = virtual_list::<(), _>(names.len(), 28.0, move |i| label(names[i].clone()).padding_x(12.0))
///     .fill_height();
/// ```
///
/// Rows are keyed by their index unless they carry a key of their own, so a row keeps its state while
/// it stays in view. The list's own gap is not used: put spacing inside the rows.
pub fn virtual_list<M: 'static, E: IntoElement<M>>(count: usize, row_height: f32, build: impl Fn(usize) -> E + 'static) -> VirtualList<M> {
    let mut base = Base::new();
    base.style.overflow = Overflow::Scroll;
    base.style.min_height = Len::Px(0.0);
    let build: Rc<dyn Fn(usize) -> Element<M>> = Rc::new(move |i| build(i).into_element());
    VirtualList { base, lazy: Lazy { count, extent: row_height.max(1.0), overscan: 4, build } }
}
