//! The layout engine against a small tree of its own, with text measured by a fake font.

use vui_core::{Rect, rect};

use super::*;
use crate::style::{Align, Justify, Len, Overflow, Styled, pct};

#[derive(Default)]
struct Cx {
    measures: usize,
}

/// A tree to lay out. Text is 10 px per character and 10 px per line, so every expected number
/// below can be worked out by hand.
struct T {
    style: Style,
    kids: Vec<T>,
    text: Option<&'static str>,
    natural: Option<Size>,
    base: Option<f32>,
    cache: LayoutCache,
    layout: NodeLayout,
}

impl T {
    fn new() -> T {
        T { style: Style::default(), kids: Vec::new(), text: None, natural: None, base: None, cache: LayoutCache::default(), layout: NodeLayout::default() }
    }

    fn kids(mut self, kids: Vec<T>) -> T {
        self.kids = kids;
        self
    }

    fn clear_caches(&mut self) {
        self.cache.clear();
        self.kids.iter_mut().for_each(T::clear_caches);
    }
}

impl Styled for T {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl LayoutNode<Cx> for T {
    fn style(&self) -> &Style {
        &self.style
    }

    fn children_mut(&mut self) -> &mut [T] {
        &mut self.kids
    }

    fn measure(&mut self, cx: &mut Cx, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        cx.measures += 1;
        if let Some(t) = self.text {
            return text_size(t, known, avail);
        }
        if let Some(s) = self.natural {
            return size(known.x.unwrap_or(s.w), known.y.unwrap_or(s.h));
        }
        Size::ZERO
    }

    fn baseline(&self) -> Option<f32> {
        self.base
    }

    fn cache(&mut self) -> &mut LayoutCache {
        &mut self.cache
    }

    fn layout_mut(&mut self) -> &mut NodeLayout {
        &mut self.layout
    }
}

fn text_size(text: &str, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
    let limit = match (known.x, avail.x) {
        (Some(w), _) => w,
        (None, Avail::Definite(a)) => a,
        (None, Avail::MinContent) => 0.0,
        (None, Avail::MaxContent) => f32::INFINITY,
    };
    let (mut lines, mut line, mut widest) = (1, 0.0f32, 0.0f32);
    for word in text.split_whitespace() {
        let w = word.len() as f32 * 10.0;
        if line > 0.0 && line + 10.0 + w > limit {
            lines += 1;
            widest = widest.max(line);
            line = w;
        } else {
            line = if line > 0.0 { line + 10.0 + w } else { w };
        }
    }
    size(known.x.unwrap_or(widest.max(line)), lines as f32 * 10.0)
}

fn col() -> T {
    T::new()
}

fn row() -> T {
    T::new().row()
}

fn boxed(w: f32, h: f32) -> T {
    T::new().width(w).height(h)
}

fn txt(text: &'static str) -> T {
    let mut t = T::new();
    t.text = Some(text);
    t.base = Some(8.0);
    t
}

fn natural(w: f32, h: f32) -> T {
    let mut t = T::new();
    t.natural = Some(size(w, h));
    t
}

fn run(root: &mut T, w: f32, h: f32) -> Cx {
    let mut cx = Cx::default();
    layout_root(root, &mut cx, size(w, h));
    cx
}

fn rect_of(t: &T) -> Rect {
    rect(t.layout.origin.x, t.layout.origin.y, t.layout.size.w, t.layout.size.h)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[track_caller]
fn assert_rect(t: &T, want: Rect) {
    let got = rect_of(t);
    assert!(close(got.x, want.x) && close(got.y, want.y) && close(got.w, want.w) && close(got.h, want.h), "expected {want:?}, got {got:?}");
}

mod cache;
mod content;
mod flex;
mod grid;
