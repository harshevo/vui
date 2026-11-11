//! Focus: Tab in and out, selecting on focus, and disabled fields.

use super::*;

#[test]
fn tabbing_into_a_line_selects_it_and_tabbing_out_leaves_the_field() {
    let mut t = focused(plain);
    t.type_text("x").tab();
    assert_eq!(t.ui.focused_key(), Some("g".into()));
    let bounds = t.rect("g");
    let g = t.ui.access_tree().nodes.into_iter().find(|n| n.bounds == bounds).unwrap();
    assert_eq!(g.selection, Some((0, 6)), "all of `second`, ready to be typed over");
    t.shift_tab();
    assert_eq!(t.ui.focused_key(), Some("f".into()));
    assert_eq!(selection(&mut t), (0, 1));
}

#[test]
fn tabbing_into_a_text_area_keeps_the_caret_where_it_was() {
    let mut t = focused(area);
    t.type_text("abc").tab().shift_tab();
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn a_field_can_ask_to_be_selected_whenever_focus_comes_by_keyboard_or_code() {
    let mut t = window_with(select_all_on_focus, "hello", false);
    t.ui.focus_key("f");
    assert_eq!(selection(&mut t), (0, 5));
    t.click(at_byte("hello", 3));
    assert_eq!(selection(&mut t), (3, 3), "a click still places the caret");
}

#[test]
fn a_disabled_field_ignores_the_pointer_and_the_keys() {
    let mut t = TestUi::<Msg>::new(320.0, 220.0);
    t.view(column().padding(20.0).child(text_field::<Msg>("x").key("f").width(240.0).on_change(Msg::Text).disabled(true)));
    t.click_key("f").type_text("abc");
    assert_eq!(t.ui.focused_key(), None);
    assert!(t.messages().is_empty());
    let n = node_of(&mut t, "f");
    assert!(n.disabled && !n.focusable && n.actions.is_empty());
}
