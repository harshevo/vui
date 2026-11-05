mod common;

use common::*;
use vui_core::pt;
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Div, Overflow, Parent, Styled, column, div, row};

/// A 200x100 window filled by a list of `n` rows, 30 px each. Content is 30n tall.
fn list(n: u32) -> Div<Msg> {
    column().key("list").overflow(Overflow::Scroll).children(
        (0..n).map(|i| div().key(i).height(30.0).shrink(0.0).focusable(true).on_click(Msg::Value(i as i32))).collect::<Vec<_>>(),
    )
}

fn scroll_y(t: &mut TestUi<Msg>) -> f32 {
    t.inspect("list").scroll.y
}

#[test]
fn the_wheel_moves_content_with_a_short_glide() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.wheel(pt(100.0, 50.0), 0.0, -30.0);
    assert_eq!(scroll_y(&mut t), 0.0, "the glide has only just begun");
    assert!(t.ui.needs_frame());
    t.settle();
    assert_eq!(scroll_y(&mut t), 30.0);
    assert_eq!(t.rect(0u32).y, -30.0);
    assert_eq!(t.rect(1u32).y, 0.0);
    assert!(!t.ui.needs_frame());
}

#[test]
fn scrolling_stops_at_both_ends() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.wheel(pt(100.0, 50.0), 0.0, -5000.0).settle();
    assert_eq!(scroll_y(&mut t), 200.0, "content is 300, window 100");
    t.wheel(pt(100.0, 50.0), 0.0, 5000.0).settle();
    assert_eq!(scroll_y(&mut t), 0.0);
}

#[test]
fn a_trackpad_moves_content_exactly_as_far_as_the_fingers() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.trackpad(pt(100.0, 50.0), 0.0, -45.0);
    assert_eq!(scroll_y(&mut t), 45.0);
    assert_eq!(t.rect(0u32).y, -45.0);
}

#[test]
fn a_row_scrolls_sideways() {
    let mut t = ui(100.0, 50.0);
    t.view(
        row().key("list").overflow(Overflow::Scroll).children(
            (0..10u32).map(|i| div().key(i).width(40.0).shrink(0.0)).collect::<Vec<_>>(),
        ),
    );
    assert_eq!(t.inspect("list").scroll_max.x, 300.0);
    t.trackpad(pt(50.0, 25.0), -80.0, 0.0);
    assert_eq!(t.rect(0u32).x, -80.0);
}

#[test]
fn content_that_fits_does_not_scroll() {
    let mut t = ui(200.0, 400.0);
    t.view(list(3));
    t.wheel(pt(100.0, 50.0), 0.0, -100.0).settle();
    assert_eq!(scroll_y(&mut t), 0.0);
    assert_eq!(t.inspect("list").scroll_max.y, 0.0);
}

#[test]
fn an_inner_scroller_scrolls_first_and_then_hands_over_to_the_outer_one() {
    let mut t = ui(200.0, 100.0);
    t.view(
        column().key("outer").overflow(Overflow::Scroll).children((
            column()
                .key("inner")
                .height(50.0)
                .shrink(0.0)
                .overflow(Overflow::Scroll)
                .child(div().height(100.0).shrink(0.0)),
            div().key("rest").height(200.0).shrink(0.0),
        )),
    );
    let over_inner = pt(100.0, 25.0);
    t.wheel(over_inner, 0.0, -30.0).settle();
    assert_eq!(t.inspect("inner").scroll.y, 30.0);
    assert_eq!(t.inspect("outer").scroll.y, 0.0);
    t.wheel(over_inner, 0.0, -30.0).settle();
    assert_eq!(t.inspect("inner").scroll.y, 50.0, "the inner one is as far as it goes");
    t.wheel(over_inner, 0.0, -30.0).settle();
    assert_eq!(t.inspect("inner").scroll.y, 50.0);
    assert_eq!(t.inspect("outer").scroll.y, 30.0, "and now the outer one moves");
}

#[test]
fn hit_testing_follows_the_scrolled_content() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.trackpad(pt(100.0, 50.0), 0.0, -60.0);
    t.click(pt(100.0, 10.0));
    assert_eq!(t.messages(), [Msg::Value(2)], "row 2 is now at the top");
}

#[test]
fn tabbing_to_an_element_out_of_view_scrolls_it_in() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    for _ in 0..5 {
        t.tab();
    }
    t.settle();
    let r = t.rect(4u32);
    assert!(r.y >= 0.0 && r.bottom() <= 100.0, "row 4 is in view: {r:?}");
    assert_eq!(scroll_y(&mut t), 58.0, "scrolled until its bottom edge clears by the 8 px margin");
}

#[test]
fn shrinking_content_pulls_the_scroll_position_back() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.trackpad(pt(100.0, 50.0), 0.0, -5000.0);
    assert_eq!(scroll_y(&mut t), 200.0);
    t.view(list(5));
    assert_eq!(scroll_y(&mut t), 50.0, "150 of content in 100 leaves 50 to scroll");
}

#[test]
fn the_scrollbar_thumb_reflects_how_much_is_in_view() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    assert!(t.inspect("list").thumb_y.is_some());
    assert!(t.inspect("list").thumb_x.is_none());
    t.trackpad(pt(100.0, 50.0), 0.0, -100.0);
    let thumb = t.inspect("list").thumb_y.unwrap();
    // 96 px of track; a third of the content is in view; the thumb is halfway along its 64 px of travel.
    assert!((thumb.h - 32.0).abs() < 0.01, "{thumb:?}");
    assert!((thumb.y - (2.0 + 32.0)).abs() < 0.01, "{thumb:?}");
    assert!((thumb.right() - 198.0).abs() < 0.01, "{thumb:?}");
}

#[test]
fn a_fixed_thumb_keeps_its_length_as_content_grows() {
    let mut t = ui(200.0, 1000.0);
    t.view(list(40).fixed_thumb(0.04));
    let short = t.inspect("list").thumb_y.unwrap();
    // 4% of 996 px of track, whatever the content's length.
    assert!((short.h - 39.84).abs() < 0.01, "{short:?}");
    t.view(list(400).fixed_thumb(0.04));
    assert_eq!(t.inspect("list").thumb_y.unwrap().h, short.h);
    t.trackpad(pt(100.0, 500.0), 0.0, -100_000.0);
    let end = t.inspect("list").thumb_y.unwrap();
    assert!((end.bottom() - 998.0).abs() < 0.01, "at the end of the content the thumb reaches the track's end: {end:?}");

    let mut t = ui(200.0, 100.0);
    t.view(list(10).fixed_thumb(0.04));
    assert_eq!(t.inspect("list").thumb_y.unwrap().h, 16.0, "never shorter than can be grabbed");
}

#[test]
fn a_short_list_has_no_thumb() {
    let mut t = ui(200.0, 400.0);
    t.view(list(3));
    assert!(t.inspect("list").thumb_y.is_none());
}

#[test]
fn dragging_the_thumb_scrolls() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    t.trackpad(pt(100.0, 50.0), 0.0, -1.0);
    t.advance(0.2);
    let thumb = t.inspect("list").thumb_y.unwrap();
    let grab = pt(thumb.center().x, thumb.center().y);
    t.press(grab);
    t.move_to(pt(grab.x, grab.y + 16.0));
    // 16 px of thumb travel out of 64 free is a quarter of the 200 px that can scroll.
    assert!((scroll_y(&mut t) - 51.0).abs() < 0.5, "{}", scroll_y(&mut t));
    t.move_to(pt(grab.x - 80.0, grab.y + 5000.0));
    assert_eq!(scroll_y(&mut t), 200.0, "the drag keeps following the pointer outside the bar");
    t.release(pt(grab.x, grab.y));
    assert!(t.messages().is_empty(), "grabbing the bar clicked nothing under it");
}

#[test]
fn the_scrollbars_fade_away_when_not_wanted() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10));
    assert_eq!(t.inspect("list").bars, 0.0);
    t.wheel(pt(100.0, 50.0), 0.0, -30.0);
    t.advance(0.3);
    assert!(t.inspect("list").bars > 0.9, "{}", t.inspect("list").bars);
    t.advance(3.0);
    assert!(t.inspect("list").bars < 0.01);
    assert!(!t.ui.needs_frame());
}

#[test]
fn only_the_rows_in_view_are_painted_and_each_is_clipped_to_the_container() {
    let mut t = ui(200.0, 300.0);
    t.view(column().child(
        column().key("box").height(100.0).shrink(0.0).overflow(Overflow::Scroll).bg(vui_core::Tone::Side).children(
            (0..10u32).map(|i| div().height(30.0).shrink(0.0).bg(vui_core::Tone::Wash).key(i)).collect::<Vec<_>>(),
        ),
    ));
    let wash = t.ui.theme().palette.wash;
    let scene = t.scene();
    let rows: Vec<_> = vui_runtime::testing::quads(&scene).into_iter().filter(|q| q.fill == wash).collect();
    assert_eq!(rows.len(), 4, "rows at 0, 30, 60 and 90 touch the 100 px box; the other six do not");
    for q in rows {
        assert_eq!(q.clip, Some(vui_core::rect(0.0, 0.0, 200.0, 100.0)));
    }
    t.trackpad(pt(100.0, 50.0), 0.0, -60.0);
    let scene = t.scene();
    let rows = vui_runtime::testing::quads(&scene).into_iter().filter(|q| q.fill == wash).count();
    assert_eq!(rows, 4, "after scrolling by two rows, rows 2 to 5 are in view");
}

#[test]
fn a_following_scroller_starts_at_the_end_and_keeps_up_until_the_reader_scrolls_back() {
    let mut t = ui(200.0, 100.0);
    t.view(list(10).follow_end());
    t.settle();
    assert_eq!(scroll_y(&mut t), 200.0, "starts at the end");
    t.view(list(12).follow_end());
    t.settle();
    assert_eq!(scroll_y(&mut t), 260.0, "keeps up with new rows");
    t.trackpad(pt(100.0, 50.0), 0.0, 100.0);
    t.view(list(14).follow_end());
    t.settle();
    assert_eq!(scroll_y(&mut t), 160.0, "lets go once the reader scrolls back");
}

#[test]
fn a_float_is_clicked_above_later_siblings_and_outside_its_scroller() {
    let mut t = ui(200.0, 100.0);
    let menu = div().key("menu").float().top(20.0).left(0.0).width(100.0).height(100.0).on_click(Msg::Value(99));
    let first = div().key(0u32).height(30.0).shrink(0.0).child(menu);
    let rest = (1..10u32).map(|i| div().key(i).height(30.0).shrink(0.0).on_click(Msg::Value(i as i32)));
    t.view(column().key("list").height(40.0).overflow(Overflow::Scroll).child(first).children(rest.collect::<Vec<_>>()));
    t.settle();
    t.click(pt(50.0, 45.0));
    assert_eq!(t.messages(), vec![Msg::Value(99)], "over row 1, and below the scroller's edge");
}

#[test]
fn a_float_takes_its_contents_height_however_little_room_its_parent_has() {
    let mut t = ui(200.0, 100.0);
    let menu = column().key("menu").float().top(20.0).left(0.0).right(0.0).children(
        (0..3u32).map(|i| div().key(i).height(30.0)).collect::<Vec<_>>(),
    );
    t.view(column().child(div().key("row").height(24.0).child(menu)));
    t.settle();
    assert_eq!(t.rect("menu").h, 90.0);
    assert_eq!(t.rect(2u32).y, 80.0);
}
