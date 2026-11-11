//! Password fields.

use super::*;

#[test]
fn a_password_field_draws_bullets_and_keeps_the_text_off_the_clipboard() {
    let mut t = focused(secret);
    let clip = clipboard(&mut t);
    t.type_text("secret");
    assert_eq!(text(&t), "secret", "the app gets the real text");
    let shown = drawn(&mut t);
    assert!(shown.contains(&"••••••".to_string()) && !shown.contains(&"secret".to_string()), "{shown:?}");
    t.shortcut('a').shortcut('c').shortcut('x');
    assert_eq!(clip.get(), None);
    assert_eq!(text(&t), "secret", "cut does nothing either");
    let n = node(&mut t);
    assert_eq!((n.value.as_deref(), n.selection), (Some("••••••"), Some((0, 6))));
}

#[test]
fn a_password_field_moves_and_selects_without_showing_where_words_end() {
    let mut t = focused(secret);
    t.type_text("two words").key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (0, 0), "a word is the whole thing");
    t.double_click(pt(60.0, MID_Y));
    assert_eq!(selection(&mut t), (0, 9));
}

#[test]
fn a_password_caret_and_clicks_line_up_with_the_bullets() {
    let mut t = focused(secret);
    t.type_text("abcd");
    let bullets = "••••";
    let step = (x_of(bullets, bullets.len()) - x_of(bullets, 0)) / 4.0;
    t.click(pt(INNER_X + step * 2.0 + 1.0, MID_Y));
    assert_eq!(selection(&mut t), (2, 2));
    let scene = t.scene();
    let c = caret(&scene).expect("a caret");
    assert!((c.x - (INNER_X + step * 2.0)).abs() < 0.6);
}
