//! Content-sized nodes: text that wraps, intrinsic sizes, scroll containers and the shapes real
//! screens take.

use super::*;

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
