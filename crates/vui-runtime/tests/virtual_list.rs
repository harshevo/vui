//! Virtual lists: only the rows in view are built, the content is as tall as every row together, and
//! scrolling builds the rows that come into view.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::*;
use vui_core::pt;
use vui_runtime::testing::texts;
use vui_runtime::{Builder, Styled, div, label, virtual_list};

fn list(count: usize, built: Rc<Cell<usize>>) -> vui_runtime::VirtualList<Msg> {
    virtual_list(count, 20.0, move |_| {
        built.set(built.get() + 1);
        div()
    })
    .key("list")
    .fill_height()
}

#[test]
fn only_the_rows_in_view_are_built() {
    let built = Rc::new(Cell::new(0));
    let mut t = ui(200.0, 100.0);
    t.view(list(100_000, built.clone()));
    t.scene();
    let nodes = t.ui.node_count();
    assert!(nodes < 80, "{nodes} nodes for a hundred thousand rows");
    assert!(built.get() < 200, "{} rows built", built.get());
    assert_eq!(t.rect(0usize), vui_core::rect(0.0, 0.0, 200.0, 20.0));
    assert!(t.ui.rect_of(5_000usize).is_none());
}

#[test]
fn the_content_is_as_tall_as_every_row() {
    let mut t = ui(200.0, 100.0);
    t.view(list(1000, Rc::new(Cell::new(0))));
    t.scene();
    assert_eq!(t.inspect("list").scroll_max.y, 20_000.0 - 100.0);
}

#[test]
fn scrolling_builds_the_rows_that_come_into_view() {
    let mut t = ui(200.0, 100.0);
    t.view(list(1000, Rc::new(Cell::new(0))));
    t.scene();
    t.trackpad(pt(100.0, 50.0), 0.0, -10_000.0);
    t.scene();
    // Row 500 is at the top of the view now.
    assert_eq!(t.rect(500usize), vui_core::rect(0.0, 0.0, 200.0, 20.0));
    assert!(t.ui.rect_of(0usize).is_none(), "the rows far above are dropped");
    let nodes = t.ui.node_count();
    assert!(nodes < 30, "{nodes} nodes");
}

#[test]
fn a_rebuilt_view_rebuilds_the_rows_in_view() {
    let mut t = ui(200.0, 100.0);
    let names = |suffix: &'static str| virtual_list::<Msg, _>(50, 20.0, move |i| label(format!("row {i}{suffix}"))).key("list").fill_height();
    t.view(names(""));
    assert!(texts(&t.scene()).contains(&"row 2".to_string()));
    t.view(names(" (edited)"));
    assert!(texts(&t.scene()).contains(&"row 2 (edited)".to_string()));
}

#[test]
fn a_short_list_builds_every_row() {
    let mut t = ui(200.0, 400.0);
    t.view(list(3, Rc::new(Cell::new(0))));
    t.scene();
    assert_eq!(t.rect(2usize).y, 40.0);
    assert_eq!(t.inspect("list").scroll_max.y, 0.0);
}
