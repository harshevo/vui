mod common;

use common::*;
use vui_core::{CursorIcon, Tone, pt};
use vui_runtime::{Builder, Effect, Overflow, Parent, Styled, column, div, label, row};

/// Two 40 px boxes, one above the other, in a 200 px window: a is (10,10,180,40), b is (10,60,180,40).
fn pair() -> impl vui_runtime::IntoElement<Msg> {
    column().gap(10.0).padding(10.0).children((
        div()
            .key("a")
            .height(40.0)
            .on_click(Msg::Click("a"))
            .on_hover(|on| Msg::Hover("a", on)),
        div()
            .key("b")
            .height(40.0)
            .on_click(Msg::Click("b"))
            .on_hover(|on| Msg::Hover("b", on)),
    ))
}

#[test]
fn hover_follows_the_pointer() {
    let mut t = ui(200.0, 200.0);
    t.view(pair());
    let (a, b) = (t.center("a"), t.center("b"));
    t.move_to(a);
    assert!(t.inspect("a").hovered && !t.inspect("b").hovered);
    assert_eq!(t.messages(), [Msg::Hover("a", true)]);

    t.move_to(b);
    assert_eq!(t.messages(), [Msg::Hover("a", false), Msg::Hover("b", true)]);
    assert!(!t.inspect("a").hovered && t.inspect("b").hovered);

    t.leave();
    assert_eq!(t.messages(), [Msg::Hover("b", false)]);
    assert!(!t.inspect("b").hovered);
}

#[test]
fn hovering_a_child_hovers_its_ancestors() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(
        column().key("parent").height(50.0).on_hover(|on| Msg::Hover("parent", on)).child(
            div().key("child").height(20.0).on_hover(|on| Msg::Hover("child", on)),
        ),
    ));
    let c = t.center("child");
    t.move_to(c);
    assert!(t.inspect("parent").hovered && t.inspect("child").hovered);
    assert_eq!(t.messages(), [Msg::Hover("parent", true), Msg::Hover("child", true)]);
    // Moving to the parent's own area leaves only the child.
    t.move_to(pt(100.0, 40.0));
    assert_eq!(t.messages(), [Msg::Hover("child", false)]);
}

#[test]
fn a_click_is_sent_when_the_button_is_released_over_the_element() {
    let mut t = ui(200.0, 200.0);
    t.view(pair());
    let a = t.center("a");
    t.press(a);
    assert!(t.inspect("a").pressed);
    assert_eq!(t.messages(), [Msg::Hover("a", true)], "no click yet");
    t.release(a);
    assert!(!t.inspect("a").pressed);
    assert_eq!(t.messages(), [Msg::Click("a")]);
}

#[test]
fn letting_go_somewhere_else_cancels_the_click() {
    let mut t = ui(200.0, 200.0);
    t.view(pair());
    let (a, b) = (t.center("a"), t.center("b"));
    t.press(a);
    t.release(b);
    let sent = t.messages();
    assert!(!sent.contains(&Msg::Click("a")) && !sent.contains(&Msg::Click("b")), "{sent:?}");
}

#[test]
fn the_pressed_look_follows_the_pointer_out_and_back() {
    let mut t = ui(200.0, 200.0);
    t.view(pair());
    let (a, b) = (t.center("a"), t.center("b"));
    t.press(a);
    assert!(t.inspect("a").pressed);
    t.move_to(b);
    assert!(!t.inspect("a").pressed, "pointer is outside");
    t.move_to(a);
    assert!(t.inspect("a").pressed, "and back inside");
    t.release(a);
    assert!(t.messages().contains(&Msg::Click("a")));
}

#[test]
fn on_press_fires_when_the_button_goes_down() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("a").height(40.0).on_press(Msg::Press("a")).on_click(Msg::Click("a"))));
    let a = t.center("a");
    t.press(a);
    assert_eq!(t.messages(), [Msg::Press("a")]);
    t.release(a);
    assert_eq!(t.messages(), [Msg::Click("a")]);
}

#[test]
fn a_double_click_sends_both_clicks_and_then_the_double() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("a").height(40.0).on_click(Msg::Click("a")).on_double_click(Msg::Double("a"))));
    let a = t.center("a");
    t.double_click(a);
    assert_eq!(t.messages(), [Msg::Click("a"), Msg::Click("a"), Msg::Double("a")]);
}

#[test]
fn a_right_click_opens_a_menu_on_the_way_down() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(
        div().key("a").height(40.0).on_click(Msg::Click("a")).on_secondary(|c| {
            assert_eq!(c.pos, pt(100.0, 20.0));
            Msg::Menu("a")
        }),
    ));
    t.right_click(pt(100.0, 20.0));
    assert_eq!(t.messages(), [Msg::Menu("a")]);
}

#[test]
fn click_handlers_see_where_the_click_was() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("a").height(40.0).on_click_at(|c| Msg::Value(c.pos.x as i32))));
    t.click(pt(77.0, 10.0));
    assert_eq!(t.messages(), [Msg::Value(77)]);
}

#[test]
fn the_topmost_sibling_gets_the_click() {
    let mut t = ui(300.0, 300.0);
    t.view(column().children((
        div().key("under").absolute().top(0.0).left(0.0).size(100.0, 100.0).on_click(Msg::Click("under")),
        div().key("over").absolute().top(50.0).left(50.0).size(100.0, 100.0).on_click(Msg::Click("over")),
    )));
    t.click(pt(75.0, 75.0));
    assert_eq!(t.messages(), [Msg::Click("over")]);
    t.click(pt(25.0, 25.0));
    assert_eq!(t.messages(), [Msg::Click("under")]);
}

#[test]
fn a_click_on_content_reaches_the_clickable_ancestor() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(
        row().key("row").height(40.0).on_click(Msg::Click("row")).child(label("Open").key("text")),
    ));
    let text = t.center("text");
    t.click(text);
    assert_eq!(t.messages(), [Msg::Click("row")]);
}

#[test]
fn the_innermost_clickable_wins() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(
        column().key("outer").height(100.0).on_click(Msg::Click("outer")).child(
            div().key("inner").height(40.0).on_click(Msg::Click("inner")),
        ),
    ));
    t.click_key("inner");
    assert_eq!(t.messages(), [Msg::Click("inner")]);
    t.click(pt(100.0, 80.0));
    assert_eq!(t.messages(), [Msg::Click("outer")]);
}

#[test]
fn a_disabled_element_and_everything_in_it_ignores_input() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(
        column()
            .key("outer")
            .height(100.0)
            .on_click(Msg::Click("outer"))
            .disabled(true)
            .child(div().key("inner").height(40.0).on_click(Msg::Click("inner")).on_hover(|on| Msg::Hover("inner", on))),
    ));
    t.click_key("inner");
    assert!(t.messages().is_empty());
    assert!(!t.inspect("inner").hovered && !t.inspect("outer").hovered);
    assert!(t.inspect("outer").disabled);
}

#[test]
fn an_element_that_is_not_clickable_does_not_swallow_the_click_behind_it() {
    let mut t = ui(300.0, 300.0);
    t.view(column().children((
        div().key("button").height(40.0).on_click(Msg::Click("button")),
        div().key("glass").absolute().inset(0.0).pointer_events(false),
    )));
    t.click_key("button");
    assert_eq!(t.messages(), [Msg::Click("button")]);
}

#[test]
fn input_cannot_reach_what_a_clipping_parent_cuts_off() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(
        column().key("window").size(100.0, 100.0).overflow(Overflow::Hidden).child(
            div().key("big").absolute().top(50.0).left(50.0).size(100.0, 100.0).on_click(Msg::Click("big")),
        ),
    ));
    t.click(pt(120.0, 120.0));
    assert!(t.messages().is_empty(), "that part of the child is clipped away");
    t.click(pt(75.0, 75.0));
    assert_eq!(t.messages(), [Msg::Click("big")]);
}

#[test]
fn dropped_files_go_to_the_element_that_accepts_them() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(
        div().key("zone").height(100.0).drop_bg(Tone::Wash).on_drop(|paths| Msg::Dropped(paths.len())),
    ));
    let zone = t.center("zone");
    t.drag_over(zone, &["/a.png", "/b.png"]);
    t.advance(0.5);
    let scene = t.scene();
    let wash = t.ui.theme().palette.wash;
    assert!(
        vui_runtime::testing::quads(&scene).iter().any(|q| q.fill == wash),
        "the drop fill shows while files are over the zone"
    );
    t.drop_files(zone, &["/a.png", "/b.png"]);
    assert_eq!(t.messages(), [Msg::Dropped(2)]);
    t.advance(0.5);
    let scene = t.scene();
    assert!(!vui_runtime::testing::quads(&scene).iter().any(|q| q.fill == wash), "and goes away after the drop");
}

#[test]
fn files_dropped_elsewhere_are_ignored() {
    let mut t = ui(300.0, 300.0);
    t.view(column().children((
        div().key("zone").height(100.0).on_drop(|paths| Msg::Dropped(paths.len())),
        div().key("other").height(100.0),
    )));
    t.drop_files(pt(10.0, 150.0), &["/a.png"]);
    assert!(t.messages().is_empty());
}

#[test]
fn the_cursor_follows_what_the_pointer_is_over() {
    let mut t = ui(300.0, 300.0);
    t.view(column().child(div().key("link").height(40.0).cursor(CursorIcon::Pointer)));
    t.effects();
    t.move_to(pt(10.0, 10.0));
    assert_eq!(t.effects(), [Effect::Cursor(CursorIcon::Pointer)]);
    t.move_to(pt(10.0, 200.0));
    assert_eq!(t.effects(), [Effect::Cursor(CursorIcon::Arrow)]);
}

#[test]
fn hover_catches_up_when_content_moves_under_a_still_pointer() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("a").height(40.0).on_hover(|on| Msg::Hover("a", on))));
    t.move_to(pt(20.0, 20.0));
    assert!(t.inspect("a").hovered);
    t.messages();
    t.view(column().children((
        div().key("new").height(40.0).on_hover(|on| Msg::Hover("new", on)),
        div().key("a").height(40.0).on_hover(|on| Msg::Hover("a", on)),
    )));
    assert!(t.inspect("new").hovered && !t.inspect("a").hovered);
    assert_eq!(t.messages(), [Msg::Hover("a", false), Msg::Hover("new", true)]);
}

#[test]
fn a_removed_element_is_forgotten_by_hover_and_press() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("a").height(40.0).on_click(Msg::Click("a"))));
    t.press(pt(20.0, 20.0));
    t.view(column());
    t.release(pt(20.0, 20.0));
    assert!(t.messages().is_empty());
}
