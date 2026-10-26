mod common;

use common::*;
use vui_core::rect;
use vui_runtime::{Align, Builder, Dirty, Overflow, Parent, Styled, column, div, label, row, spacer};

#[test]
fn the_root_fills_the_window() {
    let mut t = ui(400.0, 300.0);
    t.view(column().key("root").child(div().key("a").height(50.0)));
    assert_eq!(t.rect("root"), rect(0.0, 0.0, 400.0, 300.0));
    assert_eq!(t.rect("a"), rect(0.0, 0.0, 400.0, 50.0));
}

#[test]
fn padding_and_gaps_place_children_in_window_coordinates() {
    let mut t = ui(400.0, 300.0);
    t.view(column().padding(10.0).gap(5.0).children((div().key("a").height(20.0), div().key("b").height(30.0))));
    assert_eq!(t.rect("a"), rect(10.0, 10.0, 380.0, 20.0));
    assert_eq!(t.rect("b"), rect(10.0, 35.0, 380.0, 30.0));
}

#[test]
fn nested_offsets_add_up() {
    let mut t = ui(400.0, 300.0);
    t.view(column().padding(10.0).child(column().padding(20.0).child(div().key("deep").height(5.0))));
    assert_eq!(t.rect("deep"), rect(30.0, 30.0, 340.0, 5.0));
}

#[test]
fn a_row_shares_its_width_with_flex_children() {
    let mut t = ui(400.0, 300.0);
    t.view(column().child(row().height(40.0).children((
        div().key("l").width(100.0),
        div().key("m").flex(1.0),
        div().key("r").width(50.0),
    ))));
    assert_eq!(t.rect("l"), rect(0.0, 0.0, 100.0, 40.0));
    assert_eq!(t.rect("m"), rect(100.0, 0.0, 250.0, 40.0));
    assert_eq!(t.rect("r"), rect(350.0, 0.0, 50.0, 40.0));
}

#[test]
fn a_spacer_pushes_siblings_apart() {
    let mut t = ui(400.0, 300.0);
    t.view(row().height(20.0).children((div().key("a").width(30.0), spacer(), div().key("b").width(30.0))));
    assert_eq!(t.rect("a").x, 0.0);
    assert_eq!(t.rect("b").x, 370.0);
}

#[test]
fn a_label_is_one_line_tall_and_as_wide_as_its_text() {
    let mut t = ui(400.0, 300.0);
    t.view(row().align_items(Align::Start).child(label("Hello").key("hello")));
    let r = t.rect("hello");
    let theme = vui_core::Theme::light();
    let fonts = vui_core::Fonts::bundled_only();
    let natural = fonts.measure("Hello", &vui_core::TextStyle::from_token(theme.types.body), None);
    assert_eq!(r.h, theme.types.body.line_height);
    assert!((r.w - natural.w).abs() < 0.01, "{} vs {}", r.w, natural.w);
}

#[test]
fn a_long_label_wraps_to_the_room_it_is_given() {
    let mut t = ui(120.0, 300.0);
    t.view(column().child(label("one two three four five six seven eight").key("long")));
    let r = t.rect("long");
    assert_eq!(r.w, 120.0);
    assert!(r.h > 21.0 * 2.0, "wrapped onto several lines, height {}", r.h);
}

#[test]
fn a_label_limited_to_one_line_clips_instead_of_growing() {
    let mut t = ui(120.0, 300.0);
    t.view(column().child(row().child(label("a very long line of text that cannot possibly fit").max_lines(1).key("one").flex(1.0))));
    let r = t.rect("one");
    assert_eq!(r.w, 120.0);
    assert_eq!(r.h, 21.0);
}

#[test]
fn hidden_elements_take_no_room() {
    let mut t = ui(400.0, 300.0);
    t.view(column().children((div().height(10.0).hidden(true).key("gone"), div().key("a").height(20.0))));
    assert_eq!(t.rect("a").y, 0.0);
    assert_eq!(t.rect("gone").w, 0.0);
}

#[test]
fn an_update_with_the_same_shape_keeps_node_state() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().child(probe(&log, "p")));
    t.scene();
    assert_eq!(take(&log), ["create p", "measure p"]);
    t.view(column().child(probe(&log, "p")));
    t.scene();
    assert_eq!(take(&log), ["update p", "measure p"]);
}

#[test]
fn removing_an_element_drops_its_state() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().child(probe(&log, "p")));
    t.scene();
    take(&log);
    t.view(column());
    assert_eq!(take(&log), ["drop p"]);
}

#[test]
fn a_different_widget_in_the_same_place_replaces_the_old_one() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().child(probe(&log, "p")));
    take(&log);
    t.view(column().child(div()));
    assert_eq!(take(&log), ["drop p"]);
}

#[test]
fn unkeyed_children_match_by_position() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().children((probe(&log, "a"), probe(&log, "b"))));
    take(&log);
    // A new first child pushes the others along, but without keys the nodes are matched by position:
    // the old "b" node is reused for the new "a", and the old "a" node is dropped.
    t.view(column().children((div(), probe(&log, "a"), probe(&log, "b"))));
    assert_eq!(take(&log), ["update a", "create b", "drop a"]);
}

#[test]
fn keyed_children_survive_reordering() {
    let log = log();
    let mut t = ui(300.0, 200.0);
    let list = |order: [&'static str; 3]| {
        row().align_items(Align::Start).children(order.map(|k| probe(&log, k).key(k).width(40.0).height(20.0)))
    };
    t.view(list(["a", "b", "c"]));
    t.scene();
    assert_eq!(t.rect("a").x, 0.0);
    assert_eq!(t.rect("c").x, 80.0);
    take(&log);

    t.view(list(["c", "a", "b"]));
    t.scene();
    let entries = take(&log);
    assert!(entries.iter().all(|e| e.starts_with("update") || e.starts_with("measure")), "no node was rebuilt: {entries:?}");
    assert_eq!(t.rect("c").x, 0.0);
    assert_eq!(t.rect("a").x, 40.0);
    assert_eq!(t.rect("b").x, 80.0);
}

#[test]
fn a_keyed_node_keeps_its_identity_when_its_neighbours_come_and_go() {
    let log = log();
    let mut t = ui(300.0, 200.0);
    t.view(column().children((probe(&log, "x").key("x"), probe(&log, "y").key("y"))));
    take(&log);
    t.view(column().children((probe(&log, "y").key("y"),)));
    assert_eq!(take(&log), ["update y", "drop x"]);
}

#[test]
fn nothing_is_measured_again_when_nothing_changed() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    let view = |log: &Log| column().child(probe(log, "p").dirty(Dirty::None));
    t.view(view(&log));
    t.scene();
    take(&log);
    t.view(view(&log));
    t.scene();
    assert_eq!(take(&log), ["update p"], "no measure: the cached layout was reused");
}

#[test]
fn a_paint_only_change_repaints_without_measuring() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().child(probe(&log, "p").dirty(Dirty::Paint)));
    t.scene();
    take(&log);
    assert!(!t.ui.needs_paint());
    t.view(column().child(probe(&log, "p").dirty(Dirty::Paint)));
    assert!(t.ui.needs_paint());
    t.scene();
    assert_eq!(take(&log), ["update p"]);
}

#[test]
fn a_layout_change_measures_again() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().child(probe(&log, "p").dirty(Dirty::Layout)));
    t.scene();
    take(&log);
    t.view(column().child(probe(&log, "p").dirty(Dirty::Layout)));
    t.scene();
    assert_eq!(take(&log), ["update p", "measure p"]);
}

#[test]
fn a_sibling_is_not_measured_when_only_another_one_changes() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    let view = |dirty| {
        column().align_items(Align::Start).children((
            probe(&log, "quiet").dirty(Dirty::None).key("quiet"),
            probe(&log, "busy").dirty(dirty).key("busy"),
        ))
    };
    t.view(view(Dirty::None));
    t.scene();
    take(&log);
    t.view(view(Dirty::Layout));
    t.scene();
    let entries = take(&log);
    assert!(entries.contains(&"measure busy".to_string()), "{entries:?}");
    assert!(!entries.contains(&"measure quiet".to_string()), "{entries:?}");
}

#[test]
fn a_new_root_kind_starts_over() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(probe(&log, "root"));
    take(&log);
    t.view(div());
    assert_eq!(take(&log), ["drop root"]);
}

#[test]
fn the_window_can_be_resized() {
    let mut t = ui(400.0, 300.0);
    t.view(column().key("root").child(div().key("a").height(10.0)));
    assert_eq!(t.rect("a").w, 400.0);
    t.ui.set_size(vui_core::size(250.0, 100.0));
    assert_eq!(t.rect("root"), rect(0.0, 0.0, 250.0, 100.0));
    assert_eq!(t.rect("a").w, 250.0);
}

#[test]
fn scroll_containers_lay_children_out_at_their_natural_size() {
    let mut t = ui(200.0, 100.0);
    t.view(
        column()
            .overflow(Overflow::Scroll)
            .key("list")
            .children((0..10u32).map(|i| div().key(i).height(30.0).shrink(0.0)).collect::<Vec<_>>()),
    );
    assert_eq!(t.rect(9u32).y, 270.0);
    assert_eq!(t.inspect("list").scroll_max.y, 200.0);
}
