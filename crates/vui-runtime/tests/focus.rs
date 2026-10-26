mod common;

use common::*;
use vui_core::{Key as KeyCode, Modifiers, NamedKey, pt};
use vui_runtime::testing::quads;
use vui_runtime::{Builder, Focus, IntoElement, Parent, Styled, column, div};

/// Four 30 px rows. "three" is disabled; "two" is only reached by Tab.
fn form() -> impl IntoElement<Msg> {
    column().gap(10.0).padding(10.0).children((
        div().key("one").height(30.0).focusable(true).on_click(Msg::Click("one")),
        div().key("two").height(30.0).focus_mode(Focus::Keyboard).on_click(Msg::Click("two")),
        div().key("three").height(30.0).focusable(true).disabled(true).on_click(Msg::Click("three")),
        div().key("four").height(30.0).focusable(true).on_click(Msg::Click("four")),
    ))
}

fn focused(t: &mut vui_runtime::testing::TestUi<Msg>) -> Option<String> {
    t.ui.focused_key().map(|k| match k {
        vui_runtime::Key::Name(n) => n.to_string(),
        other => format!("{other:?}"),
    })
}

#[test]
fn tab_visits_focusable_elements_in_order_and_wraps() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    assert_eq!(focused(&mut t), None);
    let mut seen = Vec::new();
    for _ in 0..5 {
        t.tab();
        seen.push(focused(&mut t).unwrap());
    }
    assert_eq!(seen, ["one", "two", "four", "one", "two"], "the disabled row is skipped");
}

#[test]
fn shift_tab_goes_the_other_way() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    let mut seen = Vec::new();
    for _ in 0..4 {
        t.shift_tab();
        seen.push(focused(&mut t).unwrap());
    }
    assert_eq!(seen, ["four", "two", "one", "four"]);
}

#[test]
fn hidden_elements_are_skipped_by_tab() {
    let mut t = ui(200.0, 300.0);
    t.view(column().children((
        div().key("a").height(20.0).focusable(true),
        div().key("b").height(20.0).focusable(true).hidden(true),
        div().key("c").height(20.0).focusable(true),
    )));
    t.tab().tab();
    assert_eq!(focused(&mut t).as_deref(), Some("c"));
}

#[test]
fn enter_and_space_press_the_focused_element() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.tab();
    t.key(NamedKey::Enter);
    t.key(NamedKey::Space);
    assert_eq!(t.messages(), [Msg::Click("one"), Msg::Click("one")]);
}

#[test]
fn a_disabled_element_cannot_be_activated_from_the_keyboard() {
    let mut t = ui(200.0, 300.0);
    t.view(column().child(div().key("d").height(30.0).focusable(true).disabled(true).on_click(Msg::Click("d"))));
    t.tab().key(NamedKey::Enter);
    assert!(t.messages().is_empty());
    assert_eq!(focused(&mut t), None);
}

#[test]
fn clicking_a_text_field_style_element_focuses_it_without_a_ring() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.click_key("one");
    assert_eq!(focused(&mut t).as_deref(), Some("one"));
    assert!(!t.inspect("one").focus_visible, "a ring after a click would be noise");
    t.tab();
    assert_eq!(focused(&mut t).as_deref(), Some("two"));
    assert!(t.inspect("two").focus_visible, "but after Tab the ring shows");
}

#[test]
fn clicking_a_button_leaves_focus_where_it_was() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.click_key("one");
    t.messages();
    t.click_key("two");
    assert_eq!(focused(&mut t).as_deref(), Some("one"), "the toolbar button does not steal the caret");
    assert_eq!(t.messages(), [Msg::Click("two")]);
}

#[test]
fn clicking_empty_space_gives_focus_up() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.click_key("one");
    assert!(focused(&mut t).is_some());
    t.click(pt(100.0, 290.0));
    assert_eq!(focused(&mut t), None);
}

#[test]
fn keys_bubble_from_the_focused_element_to_its_ancestors() {
    let mut t = ui(200.0, 300.0);
    t.view(column().key("panel").on_key(|k| match k.key {
        KeyCode::Named(NamedKey::Escape) => Some(Msg::Key("escape")),
        _ => None,
    }).child(div().key("field").height(30.0).focusable(true)));
    t.tab();
    t.key(NamedKey::Escape);
    assert_eq!(t.messages(), [Msg::Key("escape")]);
    t.type_text("x");
    assert!(t.messages().is_empty(), "the handler passed on keys it does not want");
}

#[test]
fn shortcuts_reach_the_root_when_nothing_is_focused() {
    let mut t = ui(200.0, 300.0);
    t.view(column().on_key(|k| (k.key == KeyCode::Character('k') && k.mods.cmd).then_some(Msg::Key("palette"))));
    t.shortcut('k');
    assert_eq!(t.messages(), [Msg::Key("palette")]);
}

#[test]
fn tab_with_a_command_key_is_left_alone() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.key_with(NamedKey::Tab, Modifiers::CMD);
    assert_eq!(focused(&mut t), None);
}

#[test]
fn focus_is_forgotten_when_the_element_goes() {
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.tab();
    assert!(focused(&mut t).is_some());
    t.view(column().child(div().key("other").height(30.0).focusable(true)));
    assert_eq!(focused(&mut t), None);
    t.tab();
    assert_eq!(focused(&mut t).as_deref(), Some("other"));
}

#[test]
fn the_focus_ring_is_drawn_for_keyboard_focus_only() {
    let ring = |t: &mut vui_runtime::testing::TestUi<Msg>| {
        t.settle();
        let scene = t.scene();
        quads(&scene).iter().filter(|q| q.border.is_some_and(|b| b.width == 2.0)).count()
    };
    let mut t = ui(200.0, 300.0);
    t.view(form());
    t.click_key("one");
    assert_eq!(ring(&mut t), 0);
    t.tab();
    assert_eq!(ring(&mut t), 1);
    t.click(pt(100.0, 290.0));
    assert_eq!(ring(&mut t), 0);
}

#[test]
fn an_element_can_opt_out_of_the_ring() {
    let mut t = ui(200.0, 300.0);
    t.view(column().child(div().key("a").height(30.0).focusable(true).focus_ring(false)));
    t.tab().settle();
    let scene = t.scene();
    assert!(quads(&scene).iter().all(|q| q.border.is_none()));
}
