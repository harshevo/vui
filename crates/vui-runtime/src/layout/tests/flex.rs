//! Flex rows and columns: sizes, growing and shrinking, alignment, margins, percentages and
//! absolute children.

use super::*;

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
