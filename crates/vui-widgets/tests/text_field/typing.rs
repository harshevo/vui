//! Typing: text reaching the app, the placeholder, Return and the character limit.

use super::*;

#[test]
fn typing_sends_the_new_text_and_the_app_hands_it_back() {
    let mut t = focused(plain);
    t.type_text("hello");
    assert_eq!(text(&t), "hello");
    let n = node(&mut t);
    assert_eq!((n.role, n.value.as_deref(), n.selection), (Role::TextInput, Some("hello"), Some((5, 5))));
}

#[test]
fn the_placeholder_shows_only_while_the_field_is_empty() {
    let mut t = window(plain);
    assert!(drawn(&mut t).contains(&"Name".to_string()));
    t.click_key("f");
    assert!(drawn(&mut t).contains(&"Name".to_string()), "focus alone does not hide it");
    t.type_text("a");
    let shown = drawn(&mut t);
    assert!(shown.contains(&"a".to_string()) && !shown.contains(&"Name".to_string()), "{shown:?}");
}

#[test]
fn typing_replaces_the_selection() {
    let mut t = focused(plain);
    t.type_text("hello world").shortcut('a');
    assert_eq!(selection(&mut t), (0, 11));
    t.type_text("x");
    assert_eq!(text(&t), "x");
}

#[test]
fn a_space_is_typed_and_activates_nothing() {
    let mut t = focused(plain);
    t.type_text("a b");
    assert_eq!(text(&t), "a b");
    assert!(form(&t).submitted.is_empty());
}

#[test]
fn return_submits_a_single_line() {
    let mut t = focused(plain);
    t.type_text("hi").key(NamedKey::Enter);
    assert_eq!(form(&t).submitted, ["hi"]);
    assert_eq!(text(&t), "hi", "and leaves the text alone");
}

#[test]
fn return_adds_a_line_in_a_text_area_and_command_return_submits() {
    let mut t = focused(area);
    t.type_text("a").key(NamedKey::Enter).type_text("b");
    assert_eq!(text(&t), "a\nb");
    assert!(form(&t).submitted.is_empty());
    t.key_with(NamedKey::Enter, Modifiers::CMD);
    assert_eq!(form(&t).submitted, ["a\nb"]);
}

#[test]
fn the_character_limit_holds_for_typing_and_pasting() {
    let mut t = focused(limited);
    let clip = clipboard(&mut t);
    t.type_text("abcdefgh");
    assert_eq!(text(&t), "abcde");
    clip.set("xyz");
    t.shortcut('v');
    assert_eq!(text(&t), "abcde");
    t.key(NamedKey::Backspace).key(NamedKey::Backspace).shortcut('v');
    assert_eq!(text(&t), "abcxy", "pasting fills what room there is");
}
