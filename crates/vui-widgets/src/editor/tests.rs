    use vui_core::{Fonts, TextStyle, Theme};

    use super::*;

    fn ed(text: &str, anchor: usize, head: usize) -> Editor {
        let mut e = Editor::new();
        e.set_multiline(true);
        e.set_text(text);
        e.set_selection(anchor, head);
        e
    }

    fn caret(text: &str, at: usize) -> Editor {
        ed(text, at, at)
    }

    fn layout(text: &str, width: Option<f32>) -> TextLayout {
        let fonts = Fonts::bundled_only();
        fonts.layout(text, &TextStyle::from_token(Theme::light().types.body), width)
    }

    // ---- inserting and deleting

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

    // ---- moving

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

    // ---- lines

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

    // ---- history

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

    // ---- input methods

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
