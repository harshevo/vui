//! Moving the caret by character, word, line and paragraph, and where clicks land.

use super::*;

#[test]
fn arrows_step_over_graphemes() {
    let text = "a👨‍👩‍👧b";
    let mut e = caret(text, 0);
    let mut stops = vec![0];
    for _ in 0..3 {
        e.move_caret(Dir::Forward, Unit::Grapheme, false);
        stops.push(e.caret());
    }
    assert_eq!(stops, [0, 1, 1 + "👨‍👩‍👧".len(), text.len()]);
    e.move_caret(Dir::Forward, Unit::Grapheme, false);
    assert_eq!(e.caret(), text.len(), "the end stays the end");
    e.move_caret(Dir::Back, Unit::Grapheme, false);
    assert_eq!(e.caret(), 1 + "👨‍👩‍👧".len());
}

#[test]
fn a_plain_arrow_collapses_a_selection_to_its_edge_without_moving_past_it() {
    let mut e = ed("hello world", 2, 8);
    e.move_caret(Dir::Back, Unit::Grapheme, false);
    assert_eq!((e.caret(), e.anchor()), (2, 2));
    let mut e = ed("hello world", 8, 2);
    e.move_caret(Dir::Forward, Unit::Grapheme, false);
    assert_eq!((e.caret(), e.anchor()), (8, 8));
}

#[test]
fn shift_extends_from_the_anchor_either_way() {
    let mut e = caret("hello", 2);
    e.move_caret(Dir::Forward, Unit::Grapheme, true);
    e.move_caret(Dir::Forward, Unit::Grapheme, true);
    assert_eq!((e.anchor(), e.caret(), e.selection()), (2, 4, 2..4));
    for _ in 0..4 {
        e.move_caret(Dir::Back, Unit::Grapheme, true);
    }
    assert_eq!((e.anchor(), e.caret(), e.selection()), (2, 0, 0..2), "the caret crossed the anchor");
    assert_eq!(e.selected_text(), "he");
}

#[test]
fn word_moves_land_on_word_edges() {
    let mut e = caret("one, two  three", 0);
    let mut stops = Vec::new();
    for _ in 0..3 {
        e.move_caret(Dir::Forward, Unit::Word, false);
        stops.push(e.caret());
    }
    assert_eq!(stops, [3, 8, 15], "to the end of each word");
    let mut stops = Vec::new();
    for _ in 0..3 {
        e.move_caret(Dir::Back, Unit::Word, false);
        stops.push(e.caret());
    }
    assert_eq!(stops, [10, 5, 0], "to the start of each word");
}

#[test]
fn a_word_move_with_no_words_goes_to_the_end() {
    let mut e = caret("... !!", 1);
    e.move_caret(Dir::Forward, Unit::Word, false);
    assert_eq!(e.caret(), 6);
    e.move_caret(Dir::Back, Unit::Word, false);
    assert_eq!(e.caret(), 0);
}

#[test]
fn paragraph_and_document_moves() {
    let mut e = caret("ab\ncd\nef", 4);
    e.move_caret(Dir::Back, Unit::Paragraph, false);
    assert_eq!(e.caret(), 3);
    e.move_caret(Dir::Forward, Unit::Paragraph, false);
    assert_eq!(e.caret(), 5);
    e.move_caret(Dir::Back, Unit::Document, true);
    assert_eq!((e.anchor(), e.caret()), (5, 0));
    e.move_caret(Dir::Forward, Unit::Document, false);
    assert_eq!((e.anchor(), e.caret()), (8, 8));
}

#[test]
fn selecting_by_double_and_triple_click() {
    let mut e = caret("hello big world", 0);
    e.select_word_at(7);
    assert_eq!(e.selected_text(), "big");
    e.select_word_at(5);
    assert_eq!(e.selected_text(), " ", "between words it takes the space");
    e.select_word_at(15);
    assert_eq!(e.selected_text(), "world", "at the very end, the last word");
    let mut e = caret("one\ntwo three\nfour", 0);
    e.select_paragraph_at(6);
    assert_eq!(e.selected_text(), "two three");
    e.select_paragraph_at(0);
    assert_eq!(e.selected_text(), "one");
    e.select_paragraph_at(18);
    assert_eq!(e.selected_text(), "four");
    e.select_all();
    assert_eq!(e.selection(), 0..18);
}

#[test]
fn a_double_click_on_a_line_break_selects_nothing() {
    let mut e = caret("a\n\nb", 0);
    e.select_word_at(1);
    assert_eq!(e.selection(), 1..1);
    e.select_word_at(2);
    assert_eq!(e.selection(), 2..2, "an empty line has no word");
    assert_eq!(caret("", 0).word_range_at(0), 0..0);
}

#[test]
fn a_click_lands_on_the_letter_under_it_not_the_nearest_edge() {
    let text = "hello world";
    let l = layout(text, None);
    let x = |b: usize| l.caret_rect(b).x;
    let y = 5.0;
    assert_eq!(char_at(text, &l, pt(x(0) + 1.0, y)), 0);
    assert_eq!(char_at(text, &l, pt(x(5) - 1.0, y)), 4, "the right half of the o is still the o");
    assert_eq!(char_at(text, &l, pt(x(5) + 1.0, y)), 5, "and just past it is the space");
    assert_eq!(char_at(text, &l, pt(x(11) + 50.0, y)), 10, "past the end it is the last letter");
    assert_eq!(char_at(text, &l, pt(-20.0, y)), 0, "and before the start the first");
    assert_eq!(char_at(text, &l, pt(x(5) - 1.0, -30.0)), 4, "above the line counts as on it");
}

#[test]
fn past_the_end_of_a_line_it_is_the_last_letter_on_that_line() {
    let text = "ab\n\ncd";
    let l = layout(text, None);
    let line = |n: f32| l.caret_rect(0).h * (n + 0.5);
    assert_eq!(char_at(text, &l, pt(300.0, line(0.0))), 1, "past ab");
    assert_eq!(char_at(text, &l, pt(5.0, line(1.0))), 3, "the empty line has only its break");
    assert_eq!(char_at(text, &l, pt(300.0, line(2.0))), 5, "past cd");
}

#[test]
fn at_a_soft_wrap_a_click_past_the_end_of_the_first_line_stays_on_it() {
    let text = "aaaa bbbb cccc dddd eeee";
    let l = layout(text, Some(80.0));
    assert!(l.line_count() >= 2);
    let h = l.caret_rect(0).h;
    let second_line_start = l.hit(pt(0.0, h * 1.5));
    assert!(char_at(text, &l, pt(79.0, h * 0.5)) < second_line_start);
}

#[test]
fn positions_snap_to_grapheme_boundaries() {
    let text = "ae\u{301}b";
    let mut e = caret(text, 0);
    e.set_selection(2, 100);
    assert_eq!((e.anchor(), e.caret()), (1, text.len()), "inside the é, and past the end");
}


#[test]
fn up_and_down_keep_their_column_across_short_lines() {
    let text = "hello world\nhi\nsecond line";
    let l = layout(text, None);
    let mut e = caret(text, 8);
    e.move_lines(1, false, &l);
    assert_eq!(e.caret(), 14, "the short line ends before the column");
    e.move_lines(1, false, &l);
    assert!((e.caret() as i32 - 23).abs() <= 1, "back to the column on the long one: {}", e.caret());
    e.move_lines(-2, false, &l);
    assert!((e.caret() as i32 - 8).abs() <= 1, "two lines up in one go: {}", e.caret());
}

#[test]
fn up_from_the_first_line_goes_to_the_start_and_down_from_the_last_to_the_end() {
    let text = "one\ntwo";
    let l = layout(text, None);
    let mut e = caret(text, 2);
    e.move_lines(-1, false, &l);
    assert_eq!(e.caret(), 0);
    e.move_lines(1, false, &l);
    e.move_lines(1, false, &l);
    assert_eq!(e.caret(), 7);
    e.move_lines(-1, true, &l);
    assert_eq!(e.anchor(), 7, "extending keeps the anchor");
}

#[test]
fn a_trailing_line_break_gives_a_last_empty_line_to_move_to() {
    let text = "ab\n";
    let l = layout(text, None);
    let mut e = caret(text, 1);
    e.move_lines(1, false, &l);
    assert_eq!(e.caret(), 3);
}

#[test]
fn line_edges_are_visual_lines_in_wrapped_text() {
    let text = "the quick brown fox jumps over the lazy dog";
    let l = layout(text, Some(90.0));
    assert!(l.line_count() >= 3);
    let mut e = caret(text, 22);
    e.move_line_edge(false, false, &l);
    let start = e.caret();
    assert!(start > 0 && start < 22, "start of the wrapped line: {start}");
    assert_eq!(l.caret_rect(start).y, l.caret_rect(22).y);
    e.set_selection(22, 22);
    e.move_line_edge(true, false, &l);
    let end = e.caret();
    assert!(end > 22 && end < text.len());
    assert_eq!(l.caret_rect(end).y, l.caret_rect(22).y, "the end of the line stays on the line");
    // And from there, End again does not wander onto the next line.
    e.move_line_edge(true, false, &l);
    assert_eq!(e.caret(), end);
}
