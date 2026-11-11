//! Inserting and deleting.

use super::*;

#[test]
fn typing_inserts_at_the_caret_and_moves_it() {
    let mut e = Editor::new();
    for c in "héllo".chars() {
        assert!(e.insert(&c.to_string()));
    }
    assert_eq!((e.text(), e.caret(), e.anchor()), ("héllo", 6, 6));
    e.set_selection(1, 1);
    e.insert("X");
    assert_eq!((e.text(), e.caret()), ("hXéllo", 2));
}

#[test]
fn inserting_over_a_selection_replaces_it() {
    let mut e = ed("hello world", 6, 11);
    e.insert("there");
    assert_eq!((e.text(), e.caret(), e.has_selection()), ("hello there", 11, false));
    let mut e = ed("hello world", 11, 6);
    e.insert("");
    assert_eq!(e.text(), "hello ", "a backwards selection works too");
    assert!(!Editor::new().insert(""), "nothing to do");
}

#[test]
fn a_single_line_turns_line_breaks_into_spaces_and_drops_control_characters() {
    let mut e = Editor::new();
    e.insert("a\r\nb\nc\rd\te\u{7}f\u{0}");
    assert_eq!(e.text(), "a b c d ef");
    let mut m = Editor::new();
    m.set_multiline(true);
    m.insert("a\r\nb\rc\td\u{7}");
    assert_eq!(m.text(), "a\nb\nc\td");
}

#[test]
fn the_character_limit_counts_characters_and_keeps_whole_graphemes() {
    let mut e = Editor::new();
    e.set_max_chars(Some(5));
    e.insert("abcdefgh");
    assert_eq!(e.text(), "abcde");
    assert!(!e.insert("x"), "full");
    e.set_selection(1, 3);
    e.insert("XYZ");
    assert_eq!(e.text(), "aXYde", "replacing frees room: two out, two in");
    let mut f = Editor::new();
    f.set_max_chars(Some(3));
    f.insert("ab👨‍👩‍👧");
    assert_eq!(f.text(), "ab", "a family emoji is 5 characters, and half of one is never kept");
}

#[test]
fn backspace_deletes_a_whole_grapheme() {
    for (text, left) in [("abc👍🏽", "abc"), ("abe\u{301}", "ab"), ("a🇫🇷", "a"), ("a👨‍👩‍👧", "a"), ("a\r\n", "a")] {
        let mut e = caret(text, text.len());
        assert!(e.delete(Dir::Back, Unit::Grapheme), "{text:?}");
        assert_eq!(e.text(), left, "{text:?}");
    }
    assert!(!caret("abc", 0).delete(Dir::Back, Unit::Grapheme));
}

#[test]
fn forward_delete_takes_the_grapheme_after_the_caret() {
    let mut e = caret("e\u{301}x", 0);
    e.delete(Dir::Forward, Unit::Grapheme);
    assert_eq!(e.text(), "x");
    assert!(!caret("abc", 3).delete(Dir::Forward, Unit::Grapheme));
}

#[test]
fn deleting_with_a_selection_removes_just_the_selection() {
    let mut e = ed("hello world", 2, 8);
    assert!(e.delete(Dir::Back, Unit::Word));
    assert_eq!((e.text(), e.caret()), ("herld", 2));
    let mut e = ed("hello world", 2, 8);
    assert!(e.delete(Dir::Forward, Unit::Grapheme));
    assert_eq!(e.text(), "herld");
}

#[test]
fn deleting_by_word_goes_back_to_the_start_of_the_word() {
    let mut e = caret("one two  three", 14);
    e.delete(Dir::Back, Unit::Word);
    assert_eq!(e.text(), "one two  ");
    e.delete(Dir::Back, Unit::Word);
    assert_eq!(e.text(), "one ");
    let mut e = caret("one two", 6);
    e.delete(Dir::Back, Unit::Word);
    assert_eq!(e.text(), "one o", "from the middle of a word, back to its start");
    let mut e = caret("one two", 1);
    e.delete(Dir::Forward, Unit::Word);
    assert_eq!(e.text(), "o two", "forward takes the rest of the word");
}

#[test]
fn deleting_by_paragraph_takes_a_line_break_at_the_edge() {
    let mut e = caret("ab\ncd", 4);
    e.delete(Dir::Back, Unit::Paragraph);
    assert_eq!(e.text(), "ab\nd");
    e.delete(Dir::Back, Unit::Paragraph);
    assert_eq!(e.text(), "abd", "at the start of a paragraph, the break before it goes");
    let mut e = caret("ab\ncd", 2);
    e.delete(Dir::Forward, Unit::Paragraph);
    assert_eq!(e.text(), "abcd", "at the end of a paragraph, the break after it goes");
    assert!(!caret("ab", 2).delete(Dir::Forward, Unit::Paragraph), "and at the end of the text there is nothing");
}

#[test]
fn cut_returns_what_it_removed() {
    let mut e = ed("hello world", 0, 5);
    assert_eq!(e.cut().as_deref(), Some("hello"));
    assert_eq!(e.text(), " world");
    assert_eq!(e.cut(), None, "no selection, nothing cut");
}
