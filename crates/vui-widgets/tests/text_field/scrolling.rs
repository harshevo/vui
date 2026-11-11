//! Scrolling lines and text areas, and text areas that grow.

use super::*;

#[test]
fn a_line_scrolls_to_keep_the_caret_in_view() {
    let mut t = focused(plain);
    let long = "w".repeat(40);
    t.type_text(&long);
    let scene = t.scene();
    let drawn = text_cmd(&scene, &long);
    let right = drawn.origin.x + drawn.layout.size().w;
    assert!(drawn.origin.x < INNER_X, "scrolled left");
    assert!((right - 247.0).abs() < 1.0, "the end of the text is at the right edge: {right}");
    assert!(caret(&scene).is_some_and(|c| c.x >= INNER_X && c.right() <= 248.0));
    t.key(NamedKey::Home);
    let scene = t.scene();
    assert!((text_cmd(&scene, &long).origin.x - INNER_X).abs() < 0.01, "back to the start");
}

#[test]
fn moving_the_caret_left_scrolls_it_back_into_view() {
    let mut t = focused(plain);
    let long = format!("{}{}", "a".repeat(10), "w".repeat(40));
    t.type_text(&long);
    for _ in 0..45 {
        t.key(NamedKey::ArrowLeft);
    }
    let scene = t.scene();
    let c = caret(&scene).expect("a caret");
    assert!(c.x >= INNER_X - 0.5 && c.x <= 248.0, "{}", c.x);
}

#[test]
fn a_text_area_wraps_and_scrolls_with_the_wheel() {
    let mut t = focused(area);
    for line in 0..8 {
        t.type_text(&format!("line {line}")).key(NamedKey::Enter);
    }
    let all: String = (0..8).map(|i| format!("line {i}\n")).collect();
    let scene = t.scene();
    let end = text_cmd(&scene, &all).origin.y;
    assert!(end < 28.0, "scrolled up to show the caret on the last line: {end}");
    t.wheel(pt(100.0, 60.0), 0.0, 1000.0);
    let scene = t.scene();
    assert_eq!(text_cmd(&scene, &all).origin.y, 28.0, "the wheel can scroll it back to the top");
    t.wheel(pt(100.0, 60.0), 0.0, -30.0);
    let scene = t.scene();
    assert_eq!(text_cmd(&scene, &all).origin.y, -2.0);
}

#[test]
fn a_line_does_not_use_a_vertical_wheel() {
    let mut t = focused(plain);
    t.type_text(&"w".repeat(40));
    let before = text_cmd(&t.scene(), &"w".repeat(40)).origin.x;
    t.wheel(pt(100.0, MID_Y), 0.0, 40.0);
    assert_eq!(text_cmd(&t.scene(), &"w".repeat(40)).origin.x, before);
    t.trackpad(pt(100.0, MID_Y), 30.0, 0.0);
    assert!(text_cmd(&t.scene(), &"w".repeat(40)).origin.x > before, "but a sideways swipe moves it");
}

#[test]
fn a_text_area_moves_between_lines_and_keeps_its_column() {
    let mut t = focused(area);
    t.type_text("mm").key(NamedKey::Enter).type_text("mm").key(NamedKey::Enter).type_text("mm");
    assert_eq!(selection(&mut t), (8, 8));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (5, 5));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (2, 2));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::ArrowDown).key(NamedKey::ArrowDown);
    assert_eq!(selection(&mut t), (8, 8), "the column it left is the column it comes back to");
    t.key_with(NamedKey::ArrowUp, Modifiers::ALT);
    assert_eq!(selection(&mut t), (6, 6), "option goes to the start of the paragraph");
    t.key_with(NamedKey::ArrowUp, Modifiers::CMD);
    assert_eq!(selection(&mut t), (0, 0));
}

#[test]
fn a_text_area_grows_with_its_text_up_to_its_limit() {
    let mut t = focused(grows);
    let height = |t: &mut TestUi<Msg>| t.rect("f").h;
    assert_eq!(height(&mut t), 58.0, "two lines and the padding");
    t.type_text("a").key(NamedKey::Enter).type_text("b").key(NamedKey::Enter).type_text("c");
    assert_eq!(height(&mut t), 79.0);
    for _ in 0..4 {
        t.key(NamedKey::Enter).type_text("x");
    }
    assert_eq!(height(&mut t), 100.0, "four lines at most");
}
