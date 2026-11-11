//! Input method composition.

use super::*;

#[test]
fn a_composition_shows_inside_the_text_and_commits_into_it() {
    let mut e = caret("ab", 1);
    e.set_preedit("かん", Some((0, 3)));
    assert_eq!(e.display_text(), "aかんb");
    assert_eq!(e.text(), "ab", "the real text is untouched");
    assert_eq!(e.display_caret(), 1 + 3, "the caret sits where the input method said");
    assert_eq!(e.preedit_range(), Some(1..7));
    assert_eq!((e.to_display(0), e.to_display(1), e.to_display(2)), (0, 1, 8));
    assert_eq!((e.from_display(0), e.from_display(1), e.from_display(4), e.from_display(8)), (0, 1, 1, 2), "inside the composition is the caret");
    assert!(e.commit("漢字"));
    assert_eq!((e.text(), e.caret(), e.preedit()), ("a漢字b", 7, None));
}

#[test]
fn the_caret_defaults_to_the_end_of_a_composition() {
    let mut e = Editor::new();
    e.set_preedit("abc", None);
    assert_eq!(e.display_caret(), 3);
    e.set_preedit("", None);
    assert_eq!(e.preedit(), None, "empty text ends it");
    assert_eq!(e.display_text(), "");
}

#[test]
fn starting_a_composition_replaces_the_selection() {
    let mut e = ed("hello", 1, 4);
    e.set_preedit("x", None);
    assert_eq!((e.text(), e.caret()), ("ho", 1));
    e.set_preedit("xy", None);
    assert_eq!(e.text(), "ho", "going on with it does not eat more");
    e.clear_preedit();
    e.undo();
    assert_eq!(e.text(), "hello", "and undo gets the selection back");
}

#[test]
fn a_composition_over_a_selection_undoes_as_one_step() {
    let mut e = ed("hello", 1, 4);
    e.set_preedit("か", None);
    e.commit("漢");
    assert_eq!(e.text(), "h漢o");
    assert!(e.undo());
    assert_eq!((e.text(), e.anchor(), e.caret()), ("hello", 1, 4));
    assert!(!e.undo());
}

#[test]
fn committed_words_join_the_typing_run() {
    let mut e = Editor::new();
    e.commit("今日");
    e.commit("は");
    assert_eq!(e.text(), "今日は");
    e.undo();
    assert_eq!(e.text(), "", "one step, like typing");
}

#[test]
fn a_preedit_cursor_is_clamped_to_the_preedit() {
    let mut e = Editor::new();
    e.set_preedit("é", Some((1, 99)));
    assert_eq!(e.preedit().and_then(|p| p.cursor), Some((0, 2)), "inside the é is moved to its start, past the end to the end");
}

#[test]
fn selection_counts_characters_not_bytes() {
    let e = ed("héllo→x", 1, 7);
    assert_eq!(e.selection_chars(), (1, 5));
}

#[test]
fn undo_ends_a_composition() {
    let mut e = Editor::new();
    e.insert("a");
    e.move_caret(Dir::Back, Unit::Document, false);
    e.insert("b");
    e.set_preedit("x", None);
    e.undo();
    assert_eq!((e.text(), e.preedit()), ("a", None));
}
