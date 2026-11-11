//! The dev tools: the inspector describes the element under the pointer, the stats count frames
//! and elements, and both switch from the keyboard without the app seeing the keys.

mod common;

use common::*;
use vui_core::{Key, KeyEvent, Modifiers, Tone, pt};
use vui_runtime::testing::texts;
use vui_runtime::{Builder, Parent, Styled, column, div, label};

fn view() -> impl vui_runtime::IntoElement<Msg> {
    column().padding(10.0).child(div().key("card").padding((4.0, 8.0)).width(120.0).height(40.0).bg(Tone::Side).child(label("Hi")))
}

fn chord(c: char) -> vui_core::Event {
    vui_core::Event::Key(KeyEvent::press(Key::Character(c), Modifiers { cmd: true, alt: true, ..Modifiers::NONE }))
}

#[test]
fn the_inspector_describes_what_is_under_the_pointer() {
    let mut t = ui(400.0, 300.0);
    t.view(view());
    t.ui.handle(chord('i'));
    assert!(t.ui.inspector());
    let card = t.rect("card");
    t.move_to(pt(card.x + 2.0, card.y + 2.0));
    let shown = texts(&t.scene()).join("\n");
    assert!(shown.contains("Container  \"card\""), "{shown}");
    assert!(shown.contains("120 × 40"), "{shown}");
    assert!(shown.contains("padding 4 8 4 8"), "{shown}");
    t.ui.handle(chord('i'));
    assert!(!texts(&t.scene()).iter().any(|s| s.contains("card")), "off again");
    assert!(t.messages().is_empty());
}

#[test]
fn frame_stats_show_in_the_corner() {
    let mut t = ui(400.0, 300.0);
    t.view(view());
    t.ui.handle(chord('s'));
    let shown = texts(&t.scene()).join("\n");
    assert!(shown.contains("idle") && shown.contains("3 elements"), "{shown}");
}
