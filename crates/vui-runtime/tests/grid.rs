//! Grids and container queries inside a running UI: cells follow the window, and components restyle
//! for the room they are given rather than the window's width.

mod common;

use common::*;
use vui_core::{rect, size};
use vui_runtime::{Breakpoint, Builder, Columns, Parent, Styled, Width, column, div, grid, row};

fn cards(n: usize) -> vui_runtime::Div<Msg> {
    grid(Columns::fill(200.0)).key("grid").gap(10.0).padding(10.0).children(
        (0..n).map(|i| div().key(format!("card{i}")).height(50.0)).collect::<Vec<_>>(),
    )
}

#[test]
fn a_filling_grid_reflows_as_the_window_narrows() {
    let mut t = ui(900.0, 600.0);
    t.view(cards(5));
    // 880 of content: four columns of 212.5.
    assert_eq!(t.rect("card3"), rect(677.5, 10.0, 212.5, 50.0));
    assert_eq!(t.rect("card4"), rect(10.0, 70.0, 212.5, 50.0));

    t.ui.set_size(size(430.0, 600.0));
    assert_eq!(t.rect("card1"), rect(220.0, 10.0, 200.0, 50.0), "two columns");
    t.ui.set_size(size(300.0, 600.0));
    assert_eq!(t.rect("card1"), rect(10.0, 70.0, 280.0, 50.0), "one column");
}

#[test]
fn a_breakpoint_can_change_the_columns() {
    let mut t = ui(1000.0, 400.0);
    t.view(grid(1).key("g").when(Breakpoint::Wide, |s| s.grid(3)).children((div().key("a").height(10.0), div().key("b").height(10.0))));
    assert!((t.rect("b").x - 1000.0 / 3.0).abs() < 0.01, "{:?}", t.rect("b"));
    t.ui.set_size(size(500.0, 400.0));
    assert_eq!(t.rect("b"), rect(0.0, 10.0, 500.0, 10.0));
}

fn card(key: &str) -> vui_runtime::Div<Msg> {
    row()
        .key(key)
        .gap(10.0)
        .when_container(Width::below(300.0), |s| s.column())
        .children((div().key(format!("{key}-pic")).size(80.0, 80.0), div().key(format!("{key}-text")).height(20.0).grow(1.0)))
}

#[test]
fn a_container_rule_follows_the_room_not_the_window() {
    let mut t = ui(1000.0, 600.0);
    // The same card in a 200-wide sidebar and a wide main pane, in one window.
    t.view(row().children((column().key("side").width(200.0).child(card("narrow")), column().key("main").grow(1.0).child(card("wide")))));
    assert_eq!(t.rect("narrow-text"), rect(0.0, 90.0, 200.0, 20.0), "stacked in the sidebar");
    assert_eq!(t.rect("wide-text").y, 0.0, "side by side in the main pane");
    assert_eq!(t.rect("wide-text").x, 200.0 + 90.0);
}

#[test]
fn a_container_rule_lets_go_when_the_room_grows() {
    let mut t = ui(250.0, 600.0);
    t.view(column().child(card("c")));
    assert_eq!(t.rect("c-text").y, 90.0);
    t.ui.set_size(size(600.0, 600.0));
    assert_eq!(t.rect("c-text"), rect(90.0, 0.0, 510.0, 20.0));
    t.ui.set_size(size(280.0, 600.0));
    assert_eq!(t.rect("c-text").y, 90.0);
}
