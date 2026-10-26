mod common;

use common::*;
use vui_core::{Modifiers, NamedKey};
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Focus, FocusGroup, IntoElement, Overflow, Parent, Styled, column, div, row};

/// "before", then a vertical group of one, two and three, then "after".
fn page(group: FocusGroup) -> impl IntoElement<Msg> {
    column().children((
        div().key("before").height(20.0).focusable(true),
        column().key("group").focus_group(group).children((
            div().key("one").height(20.0).focusable(true),
            div().key("two").height(20.0).focusable(true),
            div().key("three").height(20.0).focusable(true),
        )),
        div().key("after").height(20.0).focusable(true),
    ))
}

fn focused(t: &mut TestUi<Msg>) -> Option<String> {
    t.ui.focused_key().map(|k| match k {
        vui_runtime::Key::Name(n) => n.to_string(),
        other => format!("{other:?}"),
    })
}

fn tabs(t: &mut TestUi<Msg>, n: usize) -> Vec<String> {
    (0..n)
        .map(|_| {
            t.tab();
            focused(t).unwrap()
        })
        .collect()
}

#[test]
fn a_group_is_one_tab_stop() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    assert_eq!(tabs(&mut t, 5), ["before", "one", "after", "before", "one"]);
}

#[test]
fn shift_tab_visits_the_group_once_too() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    let mut seen = Vec::new();
    for _ in 0..4 {
        t.shift_tab();
        seen.push(focused(&mut t).unwrap());
    }
    assert_eq!(seen, ["after", "one", "before", "after"]);
}

#[test]
fn arrow_keys_move_inside_the_group_and_stop_at_its_ends() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    t.tab().tab();
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
    t.key(NamedKey::ArrowUp);
    assert_eq!(focused(&mut t).as_deref(), Some("one"), "no wrapping unless asked for");
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    t.key(NamedKey::ArrowDown).key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("three"));
    t.key(NamedKey::ArrowLeft).key(NamedKey::ArrowRight);
    assert_eq!(focused(&mut t).as_deref(), Some("three"), "a vertical group ignores left and right");
}

#[test]
fn a_horizontal_group_answers_to_left_and_right() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::horizontal()));
    t.tab().tab();
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
    t.key(NamedKey::ArrowRight);
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    t.key(NamedKey::ArrowLeft);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn a_group_without_an_axis_answers_to_all_four_arrows() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::both()));
    t.tab().tab();
    t.key(NamedKey::ArrowRight).key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("three"));
    t.key(NamedKey::ArrowLeft).key(NamedKey::ArrowUp);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn a_wrapping_group_goes_round() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical().wrapping()));
    t.tab().tab();
    t.key(NamedKey::ArrowUp);
    assert_eq!(focused(&mut t).as_deref(), Some("three"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn home_and_end_jump_to_the_ends() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    t.tab().tab();
    t.key(NamedKey::End);
    assert_eq!(focused(&mut t).as_deref(), Some("three"));
    t.key(NamedKey::Home);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn arrow_navigation_shows_the_focus_ring() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    t.tab().tab().key(NamedKey::ArrowDown);
    assert!(t.inspect("two").focus_visible);
    assert!(!t.inspect("one").focused);
}

#[test]
fn leaving_and_coming_back_lands_on_the_item_that_had_focus() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    t.tab().tab().key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    t.tab();
    assert_eq!(focused(&mut t).as_deref(), Some("after"));
    t.shift_tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"), "back into the group where it was left");
    t.shift_tab().tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
}

#[test]
fn the_marked_item_is_where_tab_lands_first_and_stays() {
    let mut t = ui(200.0, 200.0);
    let view = || {
        column().children((
            div().key("before").height(20.0).focusable(true),
            column().key("group").focus_group(FocusGroup::vertical()).children((
                div().key("one").height(20.0).focusable(true),
                div().key("two").height(20.0).focusable(true).tab_default(true),
                div().key("three").height(20.0).focusable(true),
            )),
            div().key("after").height(20.0).focusable(true),
        ))
    };
    t.view(view());
    t.tab().tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"), "the marked item beats the first");
    t.key(NamedKey::ArrowDown).tab().shift_tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"), "and beats the one focus was last on");
}

#[test]
fn the_marked_item_can_change_with_the_view() {
    let mut t = ui(200.0, 200.0);
    let view = |current: &'static str| {
        column().focus_group(FocusGroup::vertical()).children(
            ["one", "two", "three"]
                .map(|k| div().key(k).height(20.0).focusable(true).tab_default(k == current))
                .into_iter()
                .collect::<Vec<_>>(),
        )
    };
    t.view(view("one")).tab();
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
    t.view(view("three")).tab().tab();
    assert_eq!(focused(&mut t).as_deref(), Some("three"));
}

#[test]
fn a_click_that_focuses_an_item_is_remembered() {
    let mut t = ui(200.0, 200.0);
    t.view(column().children((
        div().key("before").height(20.0).focusable(true),
        column().focus_group(FocusGroup::vertical()).children((
            div().key("one").height(20.0).focus_mode(Focus::Click),
            div().key("two").height(20.0).focus_mode(Focus::Click),
        )),
        div().key("after").height(20.0).focusable(true),
    )));
    t.click_key("two");
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    t.tab().shift_tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
}

#[test]
fn disabled_and_hidden_items_are_not_visited() {
    let mut t = ui(200.0, 200.0);
    t.view(column().focus_group(FocusGroup::vertical()).children((
        div().key("one").height(20.0).focusable(true),
        div().key("two").height(20.0).focusable(true).disabled(true),
        div().key("three").height(20.0).focusable(true).hidden(true),
        div().key("four").height(20.0).focusable(true),
    )));
    t.tab().key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("four"));
    t.key(NamedKey::Home);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn a_group_with_nothing_to_focus_is_not_a_stop() {
    let mut t = ui(200.0, 200.0);
    t.view(column().children((
        div().key("a").height(20.0).focusable(true),
        column().focus_group(FocusGroup::vertical()).child(div().height(20.0)),
        div().key("b").height(20.0).focusable(true),
    )));
    assert_eq!(tabs(&mut t, 3), ["a", "b", "a"]);
}

#[test]
fn a_key_handler_on_the_way_up_keeps_the_arrows_it_wants() {
    let mut t = ui(200.0, 200.0);
    t.view(column().focus_group(FocusGroup::vertical()).children((
        div().key("one").height(20.0).focusable(true),
        div().key("two").height(20.0).focusable(true).on_key(|k| {
            (k.key == vui_core::Key::Named(NamedKey::ArrowDown)).then_some(Msg::Key("down"))
        }),
        div().key("three").height(20.0).focusable(true),
    )));
    t.tab().key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("two"), "the handler took the key");
    assert_eq!(t.messages(), [Msg::Key("down")]);
    t.key(NamedKey::ArrowUp);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn arrows_with_a_modifier_are_left_alone() {
    let mut t = ui(200.0, 200.0);
    t.view(page(FocusGroup::vertical()));
    t.tab().tab();
    t.key_with(NamedKey::ArrowDown, Modifiers::SHIFT);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
    t.key_with(NamedKey::ArrowDown, Modifiers::ALT);
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
}

#[test]
fn items_in_nested_boxes_belong_to_the_group() {
    let mut t = ui(300.0, 200.0);
    t.view(column().focus_group(FocusGroup::vertical()).children((
        row().child(div().key("a").width(40.0).height(20.0).focusable(true)),
        column().padding(4.0).child(div().key("b").height(20.0).focusable(true)),
    )));
    t.tab().key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("b"));
}

#[test]
fn a_group_inside_a_group_is_a_stop_of_its_own() {
    let mut t = ui(200.0, 300.0);
    t.view(column().focus_group(FocusGroup::vertical()).children((
        div().key("outer1").height(20.0).focusable(true),
        column().focus_group(FocusGroup::vertical()).children((
            div().key("inner1").height(20.0).focusable(true),
            div().key("inner2").height(20.0).focusable(true),
        )),
        div().key("outer2").height(20.0).focusable(true),
    )));
    // The outer group's own items share one stop, the inner group has another.
    assert_eq!(tabs(&mut t, 3), ["outer1", "inner1", "outer1"]);
    // Arrow keys stay inside the nearest group: the outer group skips over what the inner one owns.
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("outer2"));
    t.tab();
    assert_eq!(focused(&mut t).as_deref(), Some("inner1"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("inner2"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focused(&mut t).as_deref(), Some("inner2"), "the inner group ends here");
}

#[test]
fn arrowing_to_an_item_that_is_scrolled_away_brings_it_into_view() {
    let mut t = ui(200.0, 100.0);
    t.view(
        column()
            .key("list")
            .overflow(Overflow::Scroll)
            .focus_group(FocusGroup::vertical())
            .children((0..10u32).map(|i| div().key(i).height(30.0).shrink(0.0).focusable(true)).collect::<Vec<_>>()),
    );
    t.tab();
    for _ in 0..5 {
        t.key(NamedKey::ArrowDown);
    }
    t.settle();
    let row = t.rect(5u32);
    assert!(row.y >= 0.0 && row.bottom() <= 100.0, "item 5 is on screen at {row:?}");
    assert!(t.inspect("list").scroll.y > 0.0);
}

#[test]
fn focus_events_say_what_moved_focus() {
    let log = log();
    let mut t = ui(200.0, 200.0);
    t.view(column().children((
        watch(&log, "a").key("a").height(20.0),
        column().focus_group(FocusGroup::vertical()).children((
            watch(&log, "b").key("b").height(20.0),
            watch(&log, "c").key("c").height(20.0),
        )),
    )));
    t.tab().tab().key(NamedKey::ArrowDown);
    t.click_key("a");
    t.ui.focus_key("c");
    assert_eq!(
        take(&log),
        [
            "a focus visible Tab",
            "a blur hidden Tab",
            "b focus visible Tab",
            "b blur hidden Arrow",
            "c focus visible Arrow",
            "c blur hidden Pointer",
            "a focus hidden Pointer",
            "a blur hidden Other",
            "c focus hidden Other",
        ]
    );
}
