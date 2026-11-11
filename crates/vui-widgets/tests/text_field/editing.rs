//! Moving the caret and deleting, with the keys macOS text fields use.

use super::*;

#[test]
fn arrows_home_and_end_move_the_caret_and_shift_extends() {
    let mut t = focused(plain);
    t.type_text("abc").key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (2, 2));
    t.key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (1, 1));
    t.key_with(NamedKey::ArrowRight, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (1, 2));
    t.key(NamedKey::Home);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::End);
    assert_eq!(selection(&mut t), (3, 3));
    t.key_with(NamedKey::Home, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (3, 0));
}

#[test]
fn a_plain_arrow_collapses_a_selection_to_its_edge() {
    let mut t = focused(plain);
    t.type_text("abcd").key_with(NamedKey::ArrowLeft, Modifiers::SHIFT).key_with(NamedKey::ArrowLeft, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (4, 2));
    t.key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (2, 2));
}

#[test]
fn backspace_and_delete_take_a_letter_each_way() {
    let mut t = focused(plain);
    t.type_text("abcd").key(NamedKey::ArrowLeft).key(NamedKey::Backspace);
    assert_eq!((text(&t).as_str(), selection(&mut t)), ("abd", (2, 2)));
    t.key(NamedKey::Delete);
    assert_eq!(text(&t), "ab");
    t.key(NamedKey::End).key(NamedKey::Delete);
    assert_eq!(text(&t), "ab", "nothing after the end");
}

#[test]
fn option_moves_and_deletes_by_word() {
    let mut t = focused(plain);
    t.type_text("one two three");
    t.key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (8, 8));
    t.key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (4, 4));
    t.key_with(NamedKey::ArrowRight, Modifiers::ALT);
    assert_eq!(selection(&mut t), (7, 7));
    t.key_with(NamedKey::Backspace, Modifiers::ALT);
    assert_eq!(text(&t), "one  three");
    t.key_with(NamedKey::Delete, Modifiers::ALT);
    assert_eq!(text(&t), "one ");
}

#[test]
fn command_arrows_and_delete_go_to_the_ends_of_the_line() {
    let mut t = focused(plain);
    t.type_text("hello world").key_with(NamedKey::ArrowLeft, Modifiers::CMD);
    assert_eq!(selection(&mut t), (0, 0));
    t.key_with(NamedKey::ArrowRight, Modifiers::CMD);
    assert_eq!(selection(&mut t), (11, 11));
    t.key_with(NamedKey::Backspace, Modifiers::CMD);
    assert_eq!(text(&t), "");
    t.type_text("ab").key(NamedKey::ArrowLeft).key_with(NamedKey::Delete, Modifiers::CMD);
    assert_eq!(text(&t), "a");
}

#[test]
fn control_keys_edit_the_way_macos_text_fields_do() {
    let mut t = focused(plain);
    t.type_text("hello");
    chord(&mut t, 'a', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (0, 0));
    chord(&mut t, 'e', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (5, 5));
    chord(&mut t, 'b', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (4, 4));
    chord(&mut t, 'f', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (5, 5));
    chord(&mut t, 'h', Modifiers::CTRL);
    assert_eq!(text(&t), "hell");
    chord(&mut t, 'a', Modifiers::CTRL);
    chord(&mut t, 'd', Modifiers::CTRL);
    assert_eq!(text(&t), "ell");
    chord(&mut t, 'k', Modifiers::CTRL);
    assert_eq!(text(&t), "");
}

#[test]
fn up_and_down_in_one_line_go_to_its_ends() {
    let mut t = focused(plain);
    t.type_text("abc").key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::ArrowDown);
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn keys_the_field_does_not_use_reach_the_elements_around_it() {
    let mut t = focused(plain);
    t.key(NamedKey::Escape).shortcut('k');
    assert_eq!(form(&t).keys, ["escape", "cmd-k"]);
    t.type_text("k");
    assert_eq!(form(&t).keys.len(), 2, "typing never gets past the field");
}
