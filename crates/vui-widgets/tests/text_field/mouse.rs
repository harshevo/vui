//! The mouse: placing the caret, dragging and multi-click selection.

use super::*;

#[test]
fn clicking_places_the_caret_between_letters() {
    let mut t = focused(plain);
    t.type_text("hello");
    t.click(at_byte("hello", 2));
    assert_eq!(selection(&mut t), (2, 2));
    t.click(pt(240.0, MID_Y));
    assert_eq!(selection(&mut t), (5, 5), "past the end of the text is the end");
    t.click(pt(21.0, MID_Y));
    assert_eq!(selection(&mut t), (0, 0), "and in the padding before it, the start");
}

#[test]
fn clicking_into_a_field_does_not_select_what_is_in_it() {
    let mut t = window_with(plain, "hello", false);
    t.click(at_byte("hello", 3));
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn dragging_selects_either_way() {
    let mut t = focused(plain);
    let s = "hello world";
    t.type_text(s);
    t.press(at_byte(s, 2)).move_to(at_byte(s, 8)).release(at_byte(s, 8));
    assert_eq!(selection(&mut t), (2, 8));
    t.press(at_byte(s, 8)).move_to(at_byte(s, 3)).release(at_byte(s, 3));
    assert_eq!(selection(&mut t), (8, 3));
}

#[test]
fn dragging_out_of_the_field_keeps_selecting() {
    let mut t = focused(plain);
    t.type_text("hello world");
    t.press(at_byte("hello world", 2)).move_to(pt(300.0, 150.0));
    assert_eq!(selection(&mut t), (2, 11), "the pointer is far away, and the end is as far as the text goes");
    t.move_to(pt(0.0, 0.0)).release(pt(0.0, 0.0));
    assert_eq!(selection(&mut t), (2, 0));
}

#[test]
fn shift_click_extends_the_selection() {
    let mut t = focused(plain);
    let s = "hello world";
    t.type_text(s).click(at_byte(s, 2));
    let p = at_byte(s, 7);
    t.ui.handle(Event::PointerDown { pos: p, button: PointerButton::Primary, mods: Modifiers::SHIFT, click_count: 1 });
    t.ui.handle(Event::PointerUp { pos: p, button: PointerButton::Primary, mods: Modifiers::SHIFT });
    assert_eq!(selection(&mut t), (2, 7));
}

#[test]
fn double_click_selects_a_word_and_triple_click_everything() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    t.double_click(at_byte(s, 7));
    assert_eq!(selection(&mut t), (6, 9));
    let p = at_byte(s, 7);
    t.press_button(p, PointerButton::Primary, 3).release(p);
    assert_eq!(selection(&mut t), (0, 15));
}

#[test]
fn double_click_on_the_right_half_of_a_words_last_letter_picks_that_word() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    let p = pt(x_of(s, 5) - 1.0, MID_Y);
    t.double_click(p);
    assert_eq!(selection(&mut t), (0, 5));
}

#[test]
fn dragging_after_a_double_click_selects_whole_words() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    let p = at_byte(s, 7);
    t.press_button(p, PointerButton::Primary, 1).release(p).press_button(p, PointerButton::Primary, 2);
    assert_eq!(selection(&mut t), (6, 9));
    let q = at_byte(s, 13);
    t.move_to(q).release(q);
    assert_eq!(selection(&mut t), (6, 15), "into the middle of world takes all of it");
    t.press_button(p, PointerButton::Primary, 1).release(p).press_button(p, PointerButton::Primary, 2);
    let r = at_byte(s, 2);
    t.move_to(r).release(r);
    assert_eq!(selection(&mut t), (9, 0), "and backwards the same way");
}

#[test]
fn the_pointer_over_a_field_is_an_i_beam() {
    let mut t = window(plain);
    t.move_to(at_byte("", 0));
    assert!(t.effects().contains(&Effect::Cursor(vui_core::CursorIcon::Text)));
}
