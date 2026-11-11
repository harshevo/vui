//! Undo and redo, and how typing groups into steps.

use super::*;

#[test]
fn typing_is_one_undo_step_and_undo_restores_the_selection_too() {
    let mut e = ed("abc", 1, 2);
    e.insert("X");
    e.insert("Y");
    e.insert("Z");
    assert_eq!(e.text(), "aXYZc");
    assert!(e.undo());
    assert_eq!((e.text(), e.anchor(), e.caret()), ("abc", 1, 2), "the replaced selection comes back with it");
}

#[test]
fn typed_runs_undo_together_but_a_caret_move_breaks_the_run() {
    let mut e = Editor::new();
    for c in "abc".chars() {
        e.insert(&c.to_string());
    }
    e.move_caret(Dir::Back, Unit::Grapheme, false);
    for c in "XY".chars() {
        e.insert(&c.to_string());
    }
    assert_eq!(e.text(), "abXYc");
    e.undo();
    assert_eq!(e.text(), "abc");
    e.undo();
    assert_eq!(e.text(), "");
    assert!(!e.undo());
}

#[test]
fn redo_replays_and_a_new_edit_forgets_it() {
    let mut e = Editor::new();
    e.insert("a");
    e.move_caret(Dir::Back, Unit::Document, false);
    e.insert("b");
    e.undo();
    assert!(e.can_redo());
    assert!(e.redo());
    assert_eq!(e.text(), "ba");
    e.undo();
    e.insert("c");
    assert!(!e.can_redo(), "typing after an undo ends the redo history");
    assert!(!e.redo());
}

#[test]
fn backspaces_undo_together_and_forward_deletes_are_their_own_run() {
    let mut e = caret("hello", 5);
    for _ in 0..3 {
        e.delete(Dir::Back, Unit::Grapheme);
    }
    e.delete(Dir::Forward, Unit::Grapheme);
    assert_eq!(e.text(), "he");
    e.undo();
    e.undo();
    assert_eq!(e.text(), "hello", "the three backspaces went back as one step, and the forward delete as another");
}

#[test]
fn replacing_the_text_from_outside_clears_history_and_keeps_the_caret_where_it_can() {
    let mut e = Editor::new();
    e.insert("hello");
    e.set_text("hello");
    assert!(e.can_undo(), "the same text is not a change");
    e.set_text("hi");
    assert!(!e.can_undo());
    assert_eq!((e.caret(), e.text()), (2, "hi"), "the caret was at the end, so it stays there");
    e.set_selection(1, 1);
    e.set_text("hello there");
    assert_eq!(e.caret(), 1, "and one in the middle stays where it was");
    e.set_text("h");
    assert_eq!(e.caret(), 1, "or lands on the end if the text no longer reaches it");
    e.set_selection(0, 1);
    e.set_text("abc");
    assert_eq!((e.anchor(), e.caret()), (0, 1), "a selection is kept as well");
}

#[test]
fn history_is_bounded() {
    let mut e = Editor::new();
    for i in 0..HISTORY + 50 {
        e.insert("x");
        e.set_selection(i, i);
    }
    let mut steps = 0;
    while e.undo() {
        steps += 1;
    }
    assert_eq!(steps, HISTORY);
}
