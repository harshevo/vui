//! Grid containers: track sizing, auto placement, spans, auto rows, filling columns, alignment.

use super::*;
use crate::style::{Columns, Track, fr};

fn grid(columns: impl Into<Columns>) -> T {
    T::new().grid(columns)
}

#[test]
fn equal_columns_share_the_width_and_wrap_into_rows() {
    let mut root = grid(3).gap(10.0).kids((0..5).map(|_| T::new().height(20.0)).collect());
    run(&mut root, 320.0, 200.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 100.0, 20.0));
    assert_rect(&root.kids[2], rect(220.0, 0.0, 100.0, 20.0));
    assert_rect(&root.kids[3], rect(0.0, 30.0, 100.0, 20.0));
    assert_rect(&root.kids[4], rect(110.0, 30.0, 100.0, 20.0));
}

#[test]
fn pixel_auto_and_fr_tracks() {
    let tracks = [Track::Px(50.0), Track::Auto, fr(1.0), fr(2.0)];
    let mut root = grid(tracks).kids(vec![boxed(10.0, 10.0), txt("abcd"), T::new(), T::new()]);
    run(&mut root, 290.0, 100.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 50.0, 10.0));
    assert_rect(&root.kids[1], rect(50.0, 0.0, 40.0, 10.0));
    assert_rect(&root.kids[2], rect(90.0, 0.0, 66.666_67, 10.0));
    assert_rect(&root.kids[3], rect(156.666_67, 0.0, 133.333_33, 10.0));
}

#[test]
fn a_row_is_as_tall_as_its_tallest_cell_and_stretches_the_rest() {
    let mut root = grid(2).kids(vec![T::new(), txt("aaaa aaaa aaaa"), T::new()]);
    run(&mut root, 100.0, 200.0);
    // 50 wide holds "aaaa" per line: three lines, 30 tall.
    assert_rect(&root.kids[0], rect(0.0, 0.0, 50.0, 30.0));
    assert_rect(&root.kids[1], rect(50.0, 0.0, 50.0, 30.0));
    assert_rect(&root.kids[2], rect(0.0, 30.0, 50.0, 0.0));
}

#[test]
fn alignment_places_a_cell_in_its_row() {
    let mut root =
        grid(3).kids(vec![T::new().height(40.0), T::new().height(10.0).align_self(Align::Center), T::new().height(10.0).align_self(Align::End)]);
    run(&mut root, 90.0, 100.0);
    assert_rect(&root.kids[1], rect(30.0, 15.0, 30.0, 10.0));
    assert_rect(&root.kids[2], rect(60.0, 30.0, 30.0, 10.0));
}

#[test]
fn a_span_takes_several_columns_and_moves_down_when_it_does_not_fit() {
    let mut root = grid(3).gap(10.0).kids(vec![T::new().height(10.0), T::new().height(10.0), T::new().height(10.0).span(2)]);
    run(&mut root, 320.0, 100.0);
    assert_rect(&root.kids[1], rect(110.0, 0.0, 100.0, 10.0));
    assert_rect(&root.kids[2], rect(0.0, 20.0, 210.0, 10.0));
}

#[test]
fn fill_makes_as_many_columns_as_fit() {
    let kids = || (0..6).map(|_| T::new().height(10.0)).collect();
    let mut wide = grid(Columns::fill(100.0)).gap(10.0).kids(kids());
    run(&mut wide, 430.0, 100.0);
    // (430 + 10) / (100 + 10) = 4 columns of 100.
    assert_rect(&wide.kids[3], rect(330.0, 0.0, 100.0, 10.0));
    assert_rect(&wide.kids[4], rect(0.0, 20.0, 100.0, 10.0));

    let mut narrow = grid(Columns::fill(100.0)).gap(10.0).kids(kids());
    run(&mut narrow, 150.0, 300.0);
    assert_rect(&narrow.kids[1], rect(0.0, 20.0, 150.0, 10.0));
}

#[test]
fn fixed_rows_and_padding() {
    let mut root = grid(2).grid_rows(24.0).padding(8.0).gap(4.0).kids((0..3).map(|_| T::new()).collect());
    run(&mut root, 116.0, 100.0);
    assert_rect(&root.kids[0], rect(8.0, 8.0, 48.0, 24.0));
    assert_rect(&root.kids[2], rect(8.0, 36.0, 48.0, 24.0));
}

#[test]
fn a_grid_without_a_width_is_as_wide_as_its_content() {
    // In a row, the grid is offered its max-content width: auto columns at their widest cell.
    let inner = grid([Track::Auto, Track::Auto]).gap(5.0).kids(vec![txt("ab"), txt("abcd"), txt("abc")]);
    let mut root = row().kids(vec![inner]);
    run(&mut root, 300.0, 100.0);
    assert_rect(&root.kids[0], rect(0.0, 0.0, 75.0, 100.0));
    assert_rect(&root.kids[0].kids[1], rect(35.0, 0.0, 40.0, 10.0));
}

#[test]
fn a_grid_is_laid_out_once_per_question() {
    let mut root = grid(2).kids((0..4).map(|_| txt("ab")).collect());
    let first = run(&mut root, 100.0, 100.0).measures;
    let mut again = Cx::default();
    layout_root(&mut root, &mut again, size(100.0, 100.0));
    assert_eq!(again.measures, 0, "a second layout of the same tree is answered from the cache");
    assert!(first <= 8, "{first} measures");
}
