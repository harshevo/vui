//! The caret and the border: blinking, placement and the focus colour.

use super::*;

#[test]
fn the_caret_blinks_and_typing_brings_it_back() {
    let mut t = focused(plain);
    assert!(caret(&t.scene()).is_some());
    t.advance(0.6);
    assert!(caret(&t.scene()).is_none(), "off for the second half second");
    t.advance(0.5);
    assert!(caret(&t.scene()).is_some(), "and on again");
    t.advance(0.5);
    assert!(caret(&t.scene()).is_none());
    t.type_text("a");
    assert!(caret(&t.scene()).is_some(), "typing shows it at once");
}

#[test]
fn the_caret_sits_after_the_last_letter() {
    let mut t = focused(plain);
    t.type_text("hello");
    let c = caret(&t.scene()).expect("a caret");
    assert!((c.x - x_of("hello", 5)).abs() <= 0.5, "{} vs {}", c.x, x_of("hello", 5));
    assert!(c.y >= t.rect("f").y && c.bottom() <= t.rect("f").bottom());
}

#[test]
fn a_selection_replaces_the_caret_and_a_read_only_field_has_none() {
    let mut t = focused(plain);
    t.type_text("hello").shortcut('a');
    assert!(caret(&t.scene()).is_none());
    let mut r = window_with(locked, "locked", false);
    r.click_key("f");
    assert!(caret(&r.scene()).is_none());
}

#[test]
fn the_caret_hides_while_the_window_is_not_active() {
    let mut t = focused(plain);
    t.window_focus(false);
    assert!(caret(&t.scene()).is_none());
    t.window_focus(true);
    assert!(caret(&t.scene()).is_some());
}

#[test]
fn the_border_goes_over_to_the_accent_with_focus() {
    let theme = Theme::light();
    let border_of = |t: &mut TestUi<Msg>| {
        let field = t.rect("f");
        let scene = t.scene();
        quads(&scene).into_iter().find(|q| q.rect == field && q.border.is_some()).and_then(|q| q.border).expect("a border").color
    };
    let near = |a: vui_core::Color, b: vui_core::Color| a.to_array().iter().zip(b.to_array()).all(|(x, y)| (x - y).abs() < 1e-3);
    let mut t = window(plain);
    assert!(near(border_of(&mut t), theme.palette.line));
    t.click_key("f").settle();
    assert!(near(border_of(&mut t), theme.palette.accent));
    t.click(pt(300.0, 200.0)).settle();
    assert!(near(border_of(&mut t), theme.palette.line), "it fades back when focus goes");
}
