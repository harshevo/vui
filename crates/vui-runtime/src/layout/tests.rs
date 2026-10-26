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

#[test]
fn a_row_places_children_with_padding_and_gap() {
    let mut root = row().padding(10.0).gap(5.0).kids(vec![boxed(20.0, 30.0), boxed(40.0, 10.0)]);
    run(&mut root, 200.0, 100.0);
    assert_rect(&root, rect(0.0, 0.0, 200.0, 100.0));
    assert_rect(&root.kids[0], rect(10.0, 10.0, 20.0, 30.0));
    assert_rect(&root.kids[1], rect(35.0, 10.0, 40.0, 10.0));
}

#[test]
fn a_column_stretches_its_children_to_its_width() {
    let mut root = col().padding(8.0).kids(vec![T::new().height(20.0), T::new().height(30.0)]);
    run(&mut root, 100.0, 200.0);
    assert_rect(&root.kids[0], rect(8.0, 8.0, 84.0, 20.0));
    assert_rect(&root.kids[1], rect(8.0, 28.0, 84.0, 30.0));
}

#[test]
fn a_column_gap_separates_vertically() {
    let mut root = col().gap(4.0).kids(vec![boxed(10.0, 10.0), boxed(10.0, 10.0), boxed(10.0, 10.0)]);
    run(&mut root, 50.0, 100.0);
    let ys: Vec<f32> = root.kids.iter().map(|k| k.layout.origin.y).collect();
    assert_eq!(ys, vec![0.0, 14.0, 28.0]);
}

#[test]
fn growing_children_share_the_free_space_by_weight() {
    let mut root = row().kids(vec![T::new().width(50.0).grow(1.0), T::new().width(50.0).grow(2.0), T::new().width(50.0)]);
    run(&mut root, 300.0, 40.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 100.0, 40.0));
    assert_rect(&root.kids[1], rect(100.0, 0.0, 150.0, 40.0));
    assert_rect(&root.kids[2], rect(250.0, 0.0, 50.0, 40.0));
}

#[test]
fn growth_stops_at_the_maximum_and_the_rest_goes_to_the_others() {
    let mut root = row().kids(vec![T::new().width(50.0).grow(1.0).max_width(80.0), T::new().width(50.0).grow(1.0)]);
    run(&mut root, 300.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 80.0));
    assert!(close(root.kids[1].layout.size.w, 220.0));
    assert!(close(root.kids[1].layout.origin.x, 80.0));
}

#[test]
fn a_total_weight_below_one_leaves_free_space() {
    let mut root = row().kids(vec![T::new().width(50.0).grow(0.25)]);
    run(&mut root, 250.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 100.0), "{}", root.kids[0].layout.size.w);
}

#[test]
fn shrinking_is_proportional_to_the_base_size() {
    let mut root = row().kids(vec![T::new().width(100.0).min_width(0.0), T::new().width(300.0).min_width(0.0)]);
    run(&mut root, 200.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 50.0));
    assert!(close(root.kids[1].layout.size.w, 150.0));
}

#[test]
fn a_shrink_of_zero_keeps_the_size() {
    let mut root = row().kids(vec![T::new().width(150.0).min_width(0.0).shrink(0.0), T::new().width(150.0).min_width(0.0)]);
    run(&mut root, 200.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 150.0));
    assert!(close(root.kids[1].layout.size.w, 50.0));
}

#[test]
fn fixed_pixel_sizes_do_not_shrink_by_default() {
    let mut root = row().kids(vec![boxed(80.0, 10.0), boxed(80.0, 10.0)]);
    run(&mut root, 100.0, 20.0);
    assert_rect(&root.kids[1], rect(80.0, 0.0, 80.0, 10.0));
    assert!(close(root.layout.content.w, 160.0), "the overflow is reported as scrollable content");
}

#[test]
fn hidden_overflow_lets_a_fixed_size_shrink() {
    let mut root = row().kids(vec![boxed(80.0, 10.0).overflow(Overflow::Hidden), boxed(80.0, 10.0).overflow(Overflow::Hidden)]);
    run(&mut root, 100.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 50.0));
    assert!(close(root.kids[1].layout.size.w, 50.0));
}

#[test]
fn text_shrinks_to_its_longest_word_and_no_further() {
    let mut root = col().align_items(Align::Start).kids(vec![row().width(60.0).kids(vec![txt("hello world"), txt("abcdef ghijkl")])]);
    run(&mut root, 300.0, 300.0);
    let r = &root.kids[0];
    assert_rect(r, rect(0.0, 0.0, 60.0, 20.0));
    assert!(close(r.kids[0].layout.size.w, 50.0));
    assert!(close(r.kids[1].layout.size.w, 60.0));
    assert!(close(r.kids[1].layout.origin.x, 50.0));
}

#[test]
fn flex_one_shares_equally_but_not_below_the_content() {
    let mut root = row().kids(vec![txt("a").flex(1.0), txt("bbbbbbbbbbbbbbbbbbbb").flex(1.0)]);
    run(&mut root, 300.0, 20.0);
    assert!(close(root.kids[0].layout.size.w, 100.0), "{}", root.kids[0].layout.size.w);
    assert!(close(root.kids[1].layout.size.w, 200.0), "{}", root.kids[1].layout.size.w);
}

#[test]
fn a_shrunk_row_gets_taller_as_its_text_wraps() {
    let mut root = col().align_items(Align::Start).kids(vec![row().width(100.0).kids(vec![txt("hello world foo")])]);
    run(&mut root, 300.0, 300.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 100.0, 20.0));
}

#[test]
fn justify_spreads_the_free_space() {
    let cases = [
        (Justify::Start, [0.0, 10.0, 20.0]),
        (Justify::Center, [35.0, 45.0, 55.0]),
        (Justify::End, [70.0, 80.0, 90.0]),
        (Justify::SpaceBetween, [0.0, 45.0, 90.0]),
        (Justify::SpaceAround, [11.667, 45.0, 78.333]),
        (Justify::SpaceEvenly, [17.5, 45.0, 72.5]),
    ];
    for (justify, xs) in cases {
        let mut root = row().justify(justify).kids(vec![boxed(10.0, 10.0), boxed(10.0, 10.0), boxed(10.0, 10.0)]);
        run(&mut root, 100.0, 20.0);
        for (k, want) in root.kids.iter().zip(xs) {
            assert!((k.layout.origin.x - want).abs() < 0.01, "{justify:?}: {} != {want}", k.layout.origin.x);
        }
    }
}

#[test]
fn centring_never_pushes_content_before_the_start_edge() {
    let mut root = row().justify(Justify::Center).kids(vec![boxed(40.0, 10.0), boxed(40.0, 10.0)]);
    run(&mut root, 50.0, 20.0);
    assert!(close(root.kids[0].layout.origin.x, 0.0));
    assert!(close(root.kids[1].layout.origin.x, 40.0));
}

#[test]
fn align_items_places_children_across() {
    let ys = |align: Align| {
        let mut root = row().align_items(align).kids(vec![boxed(10.0, 20.0), boxed(10.0, 40.0)]);
        run(&mut root, 100.0, 100.0);
        [root.kids[0].layout.origin.y, root.kids[1].layout.origin.y]
    };
    assert_eq!(ys(Align::Start), [0.0, 0.0]);
    assert_eq!(ys(Align::Center), [40.0, 30.0]);
    assert_eq!(ys(Align::End), [80.0, 60.0]);
}

#[test]
fn stretch_fills_the_line_unless_the_size_is_set() {
    let mut root = row().kids(vec![T::new().width(10.0), boxed(10.0, 30.0)]);
    run(&mut root, 100.0, 100.0);
    assert!(close(root.kids[0].layout.size.h, 100.0));
    assert!(close(root.kids[1].layout.size.h, 30.0));
}

#[test]
fn align_self_overrides_the_container() {
    let mut root = row().align_items(Align::Start).kids(vec![boxed(10.0, 20.0), boxed(10.0, 20.0).align_self(Align::End)]);
    run(&mut root, 100.0, 100.0);
    assert!(close(root.kids[0].layout.origin.y, 0.0));
    assert!(close(root.kids[1].layout.origin.y, 80.0));
}

#[test]
fn margins_take_space_around_a_child() {
    let mut root = row().kids(vec![boxed(20.0, 20.0).margin(5.0), boxed(20.0, 20.0)]);
    run(&mut root, 200.0, 100.0);
    assert_rect(&root.kids[0], rect(5.0, 5.0, 20.0, 20.0));
    assert!(close(root.kids[1].layout.origin.x, 30.0));
}

#[test]
fn an_auto_margin_pushes_a_child_to_the_far_end() {
    let mut root = row().kids(vec![boxed(20.0, 20.0), boxed(20.0, 20.0).margin_left_auto()]);
    run(&mut root, 200.0, 100.0);
    assert!(close(root.kids[1].layout.origin.x, 180.0));
}

#[test]
fn auto_margins_on_both_sides_centre_a_child() {
    let mut root = row().kids(vec![boxed(20.0, 20.0).style(|s| {
        s.margin.left = Len::Auto;
        s.margin.right = Len::Auto;
    })]);
    run(&mut root, 200.0, 100.0);
    assert!(close(root.kids[0].layout.origin.x, 90.0));
}

#[test]
fn auto_margins_work_across_the_line_too() {
    let mut both = row().kids(vec![boxed(20.0, 20.0).style(|s| {
        s.margin.top = Len::Auto;
        s.margin.bottom = Len::Auto;
    })]);
    run(&mut both, 200.0, 100.0);
    assert!(close(both.kids[0].layout.origin.y, 40.0));
    let mut top = row().kids(vec![boxed(20.0, 20.0).margin_top_auto()]);
    run(&mut top, 200.0, 100.0);
    assert!(close(top.kids[0].layout.origin.y, 80.0));
}

#[test]
fn percentages_are_of_the_parents_content_box() {
    let mut root = col().padding(10.0).kids(vec![T::new().width(pct(50.0)).height(pct(25.0))]);
    run(&mut root, 220.0, 220.0);
    assert_rect(&root.kids[0], rect(10.0, 10.0, 100.0, 50.0));
}

#[test]
fn a_percentage_of_an_undecided_size_is_auto() {
    let mid = col().width(100.0).kids(vec![T::new().width(10.0).height(pct(50.0))]);
    let mut root = col().align_items(Align::Start).kids(vec![mid]);
    run(&mut root, 200.0, 200.0);
    assert!(close(root.kids[0].layout.size.h, 0.0), "the percentage height had nothing to be a percentage of");
}

#[test]
fn max_width_limits_a_stretched_child() {
    let mut root = col().kids(vec![T::new().height(10.0).max_width(120.0)]);
    run(&mut root, 200.0, 50.0);
    assert!(close(root.kids[0].layout.size.w, 120.0));
}

#[test]
fn an_aspect_ratio_fills_in_the_missing_size() {
    let mut tile = col().kids(vec![T::new().aspect_ratio(2.0)]);
    run(&mut tile, 200.0, 400.0);
    assert_rect(&tile.kids[0], rect(0.0, 0.0, 200.0, 100.0));

    let mut set_width = col().kids(vec![T::new().width(100.0).aspect_ratio(2.0)]);
    run(&mut set_width, 200.0, 400.0);
    assert_rect(&set_width.kids[0], rect(0.0, 0.0, 100.0, 50.0));

    let mut in_row = row().kids(vec![T::new().height(30.0).aspect_ratio(2.0)]);
    run(&mut in_row, 200.0, 100.0);
    assert_rect(&in_row.kids[0], rect(0.0, 0.0, 60.0, 30.0));
}

#[test]
fn wrapping_starts_a_new_line_when_the_row_is_full() {
    let kids = (0..6).map(|_| boxed(40.0, 20.0)).collect();
    let mut root = col().align_items(Align::Start).kids(vec![row().wrap().gap(10.0).width(100.0).kids(kids)]);
    run(&mut root, 300.0, 300.0);
    let wrapped = &root.kids[0];
    assert_rect(wrapped, rect(0.0, 0.0, 100.0, 80.0));
    let at: Vec<(f32, f32)> = wrapped.kids.iter().map(|k| (k.layout.origin.x, k.layout.origin.y)).collect();
    assert_eq!(at, vec![(0.0, 0.0), (50.0, 0.0), (0.0, 30.0), (50.0, 30.0), (0.0, 60.0), (50.0, 60.0)]);
}

#[test]
fn items_on_a_wrapped_line_stretch_to_the_tallest() {
    let kids = vec![natural(20.0, 10.0), natural(20.0, 30.0), natural(20.0, 15.0)];
    let mut root = col().align_items(Align::Start).kids(vec![row().wrap().width(50.0).kids(kids)]);
    run(&mut root, 300.0, 300.0);
    let line = &root.kids[0];
    assert!(close(line.kids[0].layout.size.h, 30.0));
    assert!(close(line.kids[1].layout.size.h, 30.0));
    assert!(close(line.kids[2].layout.origin.y, 30.0));
    assert!(close(line.kids[2].layout.size.h, 15.0));
}

#[test]
fn a_baseline_row_lines_text_up() {
    let mut tall = natural(20.0, 30.0);
    tall.base = Some(18.0);
    let mut root = row().align_items(Align::Baseline).kids(vec![txt("aa"), tall]);
    run(&mut root, 200.0, 100.0);
    assert!(close(root.kids[0].layout.origin.y, 10.0), "{}", root.kids[0].layout.origin.y);
    assert!(close(root.kids[1].layout.origin.y, 0.0));
}

#[test]
fn a_container_reports_its_first_childs_baseline() {
    let mut inner = row().padding(4.0).kids(vec![txt("aa")]);
    inner.style_mut().align_items = Align::Start;
    let mut root = col().align_items(Align::Start).kids(vec![inner]);
    run(&mut root, 200.0, 100.0);
    assert_eq!(root.kids[0].layout.baseline, Some(12.0));
    assert_eq!(root.layout.baseline, Some(12.0));
}

#[test]
fn absolute_children_sit_against_the_parent_and_leave_the_flow() {
    let flow = boxed(10.0, 30.0);
    let corner = boxed(20.0, 10.0).absolute().right(5.0).bottom(5.0);
    let pinned = T::new().absolute().inset(10.0);
    let mut root = col().align_items(Align::Start).kids(vec![col().width(100.0).height(80.0).kids(vec![flow, corner, pinned])]);
    run(&mut root, 300.0, 300.0);
    let parent = &root.kids[0];
    assert_rect(&parent.kids[0], rect(0.0, 0.0, 10.0, 30.0));
    assert_rect(&parent.kids[1], rect(75.0, 65.0, 20.0, 10.0));
    assert_rect(&parent.kids[2], rect(10.0, 10.0, 80.0, 60.0));
    assert_rect(parent, rect(0.0, 0.0, 100.0, 80.0));
}

#[test]
fn an_absolute_child_without_insets_follows_the_parents_alignment() {
    let overlay = boxed(20.0, 10.0).absolute();
    let mut root = col().align_items(Align::Start).kids(vec![col().width(100.0).height(80.0).center().kids(vec![overlay])]);
    run(&mut root, 300.0, 300.0);
    assert_rect(&root.kids[0].kids[0], rect(40.0, 35.0, 20.0, 10.0));
}

#[test]
fn an_absolute_child_sizes_to_its_content_within_the_parent() {
    let label = txt("hello world hello").absolute().top(0.0).left(0.0);
    let mut root = col().align_items(Align::Start).kids(vec![col().width(100.0).height(80.0).kids(vec![label])]);
    run(&mut root, 300.0, 300.0);
    let label = &root.kids[0].kids[0];
    assert!(label.layout.size.w <= 100.0, "{}", label.layout.size.w);
    assert!(close(label.layout.size.h, 30.0), "three words of 50 px wrap to three lines in 100 px: {}", label.layout.size.h);
}

#[test]
fn hidden_children_take_no_space() {
    let mut root = row().gap(10.0).kids(vec![boxed(20.0, 20.0), boxed(20.0, 20.0).hidden(true), boxed(20.0, 20.0)]);
    run(&mut root, 200.0, 50.0);
    assert!(close(root.kids[2].layout.origin.x, 30.0));
}

#[test]
fn text_is_fitted_to_the_room_it_is_offered() {
    let mut start = col().align_items(Align::Start).kids(vec![txt("hello world")]);
    run(&mut start, 100.0, 100.0);
    assert_rect(&start.kids[0], rect(0.0, 0.0, 50.0, 20.0));

    let mut roomy = col().align_items(Align::Start).kids(vec![txt("hello world")]);
    run(&mut roomy, 200.0, 100.0);
    assert_rect(&roomy.kids[0], rect(0.0, 0.0, 110.0, 10.0));

    let mut stretched = col().kids(vec![txt("hello world")]);
    run(&mut stretched, 100.0, 100.0);
    assert_rect(&stretched.kids[0], rect(0.0, 0.0, 100.0, 20.0));
}

fn intrinsic(node: &mut T, w: Avail) -> Size {
    let input = LayoutInput { known: Pair::new(None, None), parent: Pair::new(None, None), avail: Pair::new(w, Avail::MaxContent), mode: Mode::Measure };
    compute(node, &mut Cx::default(), input).size
}

#[test]
fn a_container_adds_up_its_childrens_intrinsic_sizes() {
    let mut r = row().gap(10.0).kids(vec![txt("hello world"), txt("abcdef ghijkl")]);
    let max = intrinsic(&mut r, Avail::MaxContent);
    assert_eq!((max.w, max.h), (250.0, 10.0));
    let min = intrinsic(&mut r, Avail::MinContent);
    assert_eq!((min.w, min.h), (120.0, 20.0));

    let mut c = col().kids(vec![txt("hello world"), txt("abcdef ghijkl")]);
    assert_eq!(intrinsic(&mut c, Avail::MaxContent).w, 130.0, "a column is as wide as its widest child");
    assert_eq!(intrinsic(&mut c, Avail::MinContent).w, 60.0);
}

#[test]
fn a_wrapping_row_is_as_narrow_as_its_widest_child_when_squeezed() {
    let mut r = row().wrap().kids(vec![boxed(30.0, 10.0), boxed(50.0, 10.0), boxed(20.0, 10.0)]);
    assert_eq!(intrinsic(&mut r, Avail::MinContent).w, 50.0);
    assert_eq!(intrinsic(&mut r, Avail::MaxContent).w, 100.0);
}

#[test]
fn scroll_containers_lay_children_out_at_their_natural_size() {
    let kids = || (0..5).map(|_| T::new().height(50.0).min_height(0.0)).collect::<Vec<_>>();
    let scroll = col().width(80.0).height(100.0).padding(10.0).overflow(Overflow::Scroll).kids(kids());
    let mut root = col().align_items(Align::Start).kids(vec![scroll]);
    run(&mut root, 300.0, 300.0);
    let s = &root.kids[0];
    assert_rect(s, rect(0.0, 0.0, 80.0, 100.0));
    let ys: Vec<f32> = s.kids.iter().map(|k| k.layout.origin.y).collect();
    assert_eq!(ys, vec![10.0, 60.0, 110.0, 160.0, 210.0]);
    assert!(close(s.kids[0].layout.size.w, 60.0) && close(s.kids[0].layout.size.h, 50.0));
    assert_eq!((s.layout.content.w, s.layout.content.h), (80.0, 270.0), "padding at the end is part of the scroll range");

    // The same column without scrolling squeezes its children to fit instead.
    let plain = col().width(80.0).height(100.0).padding(10.0).kids(kids());
    let mut root = col().align_items(Align::Start).kids(vec![plain]);
    run(&mut root, 300.0, 300.0);
    assert!(close(root.kids[0].kids[0].layout.size.h, 16.0), "{}", root.kids[0].kids[0].layout.size.h);
}

#[test]
fn a_scroll_container_without_a_height_fits_the_room_it_is_offered() {
    let kids = (0..10).map(|_| boxed(50.0, 50.0)).collect();
    let mut root = col().align_items(Align::Start).kids(vec![col().overflow(Overflow::Scroll).kids(kids)]);
    run(&mut root, 100.0, 120.0);
    assert!(close(root.kids[0].layout.size.h, 120.0), "{}", root.kids[0].layout.size.h);
    assert!(close(root.kids[0].layout.content.h, 500.0));
}

#[test]
fn a_sidebar_beside_a_growing_content_column() {
    let content = col().flex(1.0).kids(vec![T::new().height(40.0), T::new().flex(1.0)]);
    let mut root = row().kids(vec![T::new().width(100.0), content]);
    run(&mut root, 400.0, 300.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 100.0, 300.0));
    assert_rect(&root.kids[1], rect(100.0, 0.0, 300.0, 300.0));
    assert_rect(&root.kids[1].kids[0], rect(0.0, 0.0, 300.0, 40.0));
    assert_rect(&root.kids[1].kids[1], rect(0.0, 40.0, 300.0, 260.0));
}

#[test]
fn a_label_and_a_control_in_a_settings_row() {
    // Label on the left, the control pushed right, both centred vertically.
    let mut root = row().align_items(Align::Center).padding(8.0).kids(vec![txt("Launch at login"), boxed(40.0, 24.0).margin_left_auto()]);
    run(&mut root, 400.0, 56.0);
    assert_rect(&root.kids[0], rect(8.0, 23.0, 150.0, 10.0));
    assert_rect(&root.kids[1], rect(352.0, 16.0, 40.0, 24.0));
}

fn tree_with_text() -> T {
    col().kids(vec![row().gap(8.0).kids(vec![txt("hello world"), txt("abc")]), txt("a longer line of text"), col().kids(vec![natural(30.0, 20.0)])])
}

#[test]
fn laying_out_again_asks_nothing_new() {
    let mut root = tree_with_text();
    let first = run(&mut root, 300.0, 200.0);
    assert!(first.measures > 0);
    let again = run(&mut root, 300.0, 200.0);
    assert_eq!(again.measures, 0, "every question had already been answered");
}

#[test]
fn clearing_the_caches_up_a_path_picks_up_a_change() {
    let mut root = tree_with_text();
    run(&mut root, 300.0, 200.0);
    let before = root.kids[0].kids[0].layout.size.w;
    assert!(close(before, 110.0), "{before}");
    root.kids[0].kids[0].text = Some("hi");
    root.cache.clear();
    root.kids[0].cache.clear();
    root.kids[0].kids[0].cache.clear();
    run(&mut root, 300.0, 200.0);
    assert!(close(root.kids[0].kids[0].layout.size.w, 20.0), "{}", root.kids[0].kids[0].layout.size.w);
}

/// A tree that doubles at every level: `2^levels` text leaves under alternating rows and columns.
fn balanced(levels: u32) -> T {
    if levels == 0 {
        return txt("ab cd");
    }
    let pair = vec![balanced(levels - 1), balanced(levels - 1)];
    if levels.is_multiple_of(2) { row().gap(2.0).kids(pair) } else { col().gap(2.0).kids(pair) }
}

fn measures_for(levels: u32) -> usize {
    let mut root = balanced(levels);
    run(&mut root, 4000.0, 4000.0).measures
}

#[test]
fn measuring_cost_grows_with_the_number_of_nodes_not_the_depth() {
    // 16 times the leaves should cost about 16 times the measures. Measuring that is exponential in
    // the depth would cost far more.
    let (shallow, deeper) = (measures_for(6), measures_for(10));
    assert!(deeper < shallow * 16 * 3, "{shallow} measures at 64 leaves, {deeper} at 1024");
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<V: Copy>(&mut self, options: &[V]) -> V {
        options[self.below(options.len() as u64) as usize]
    }
}

fn random_tree(rng: &mut Rng, depth: u32) -> T {
    let mut t = if depth == 0 || rng.chance(25) {
        match rng.below(3) {
            0 => txt(rng.pick(&["a", "hello world", "the quick brown fox", "abcdefghij klm", ""])),
            1 => natural(rng.below(60) as f32, rng.below(40) as f32),
            _ => T::new(),
        }
    } else {
        let n = 1 + rng.below(4);
        T::new().kids((0..n).map(|_| random_tree(rng, depth - 1)).collect())
    };
    if rng.chance(50) {
        t = t.row();
    }
    if rng.chance(30) {
        t = t.gap(rng.below(12) as f32);
    }
    if rng.chance(30) {
        t = t.padding(rng.below(12) as f32);
    }
    if rng.chance(20) {
        t = t.wrap();
    }
    if rng.chance(30) {
        t = t.width(rng.below(120) as f32);
    }
    if rng.chance(30) {
        t = t.height(rng.below(80) as f32);
    }
    if rng.chance(15) {
        t = t.width(pct(rng.below(100) as f32));
    }
    if rng.chance(15) {
        t = t.max_width(rng.below(150) as f32);
    }
    if rng.chance(15) {
        t = t.min_height(rng.below(50) as f32);
    }
    if rng.chance(30) {
        t = t.grow(rng.below(3) as f32);
    }
    if rng.chance(20) {
        t = t.shrink(rng.below(3) as f32);
    }
    if rng.chance(20) {
        t = t.margin(rng.below(8) as f32);
    }
    if rng.chance(10) {
        t = t.margin_left_auto();
    }
    if rng.chance(10) {
        t = t.aspect_ratio(0.5 + rng.below(4) as f32 * 0.5);
    }
    if rng.chance(8) {
        t = t.hidden(true);
    }
    if rng.chance(8) {
        t = t.absolute().top(rng.below(10) as f32).left(rng.below(10) as f32);
    }
    if rng.chance(10) {
        t = t.overflow(Overflow::Scroll);
    }
    t = t.align_items(rng.pick(&[Align::Start, Align::Center, Align::End, Align::Stretch, Align::Baseline]));
    t.justify(rng.pick(&[Justify::Start, Justify::Center, Justify::End, Justify::SpaceBetween, Justify::SpaceAround, Justify::SpaceEvenly]))
}

fn same(a: &T, b: &T) -> bool {
    a.layout == b.layout && a.kids.len() == b.kids.len() && a.kids.iter().zip(&b.kids).all(|(x, y)| same(x, y))
}

fn sane(t: &T) -> bool {
    let l = t.layout;
    let finite = [l.origin.x, l.origin.y, l.size.w, l.size.h, l.content.w, l.content.h].iter().all(|v| v.is_finite());
    finite && l.size.w >= 0.0 && l.size.h >= 0.0 && t.kids.iter().all(sane)
}

#[test]
fn random_trees_lay_out_to_finite_sizes() {
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut root = random_tree(&mut rng, 5);
        run(&mut root, 320.0, 240.0);
        assert!(sane(&root), "seed {seed}");
    }
}

#[test]
fn an_answer_from_the_cache_matches_a_fresh_one() {
    for seed in 1..=300u64 {
        let build = || random_tree(&mut Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1), 5);
        let mut worn = build();
        // Lay out at other sizes first, so the caches hold answers to other questions, then at the
        // size that is compared.
        run(&mut worn, 200.0, 500.0);
        run(&mut worn, 640.0, 120.0);
        let repeat = run(&mut worn, 320.0, 240.0).measures;
        let mut fresh = build();
        run(&mut fresh, 320.0, 240.0);
        assert!(same(&worn, &fresh), "seed {seed}: layout depends on what was asked before");
        let again = run(&mut worn, 320.0, 240.0).measures;
        assert_eq!(again, 0, "seed {seed}: asking the same thing twice measured {again} times (first {repeat})");
    }
}

#[test]
fn clearing_every_cache_changes_nothing() {
    for seed in 1..=100u64 {
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let mut root = random_tree(&mut rng, 5);
        run(&mut root, 320.0, 240.0);
        let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
        let mut other = random_tree(&mut rng, 5);
        run(&mut other, 320.0, 240.0);
        other.clear_caches();
        run(&mut other, 320.0, 240.0);
        assert!(same(&root, &other), "seed {seed}");
    }
}
