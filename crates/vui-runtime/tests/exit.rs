//! Leaving: keyed elements marked `disappear` fade out in place before they go, take no input
//! while they do, and come back if the view brings them back.

mod common;

use common::*;
use vui_core::Tone;
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Parent, Styled, column, div, label};

fn rows(show_b: bool) -> impl vui_runtime::IntoElement<Msg> {
    let row = |name: &'static str| div().key(name).height(30.0).bg(Tone::Side).disappear().on_click(Msg::Click(name)).child(label(name));
    column().children((row("a"), show_b.then(|| row("b")), row("c")))
}

fn shown(t: &mut TestUi<Msg>) -> Vec<String> {
    let scene = t.scene();
    texts(&scene)
}

#[test]
fn a_dropped_element_fades_out_in_place_then_goes() {
    let mut t = ui(200.0, 200.0);
    t.view(rows(true));
    let c_before = t.rect("c");
    t.view(rows(false));
    assert_eq!(shown(&mut t), ["a", "b", "c"], "still drawn while it fades");
    assert_eq!(t.rect("c"), c_before, "and still holds its place");
    t.advance(0.05);
    assert!(t.ui.needs_frame(), "it is animating");
    t.settle();
    assert_eq!(shown(&mut t), ["a", "c"]);
    assert!(t.rect("c").y < c_before.y, "the gap closes once it has gone");
}

#[test]
fn a_leaving_element_takes_no_clicks() {
    let mut t = ui(200.0, 200.0);
    t.view(rows(true));
    let b = t.center("b");
    t.view(rows(false));
    t.click(b);
    assert!(t.messages().is_empty(), "clicks pass through it");
}

#[test]
fn an_element_brought_back_fades_back_in() {
    let mut t = ui(200.0, 200.0);
    t.view(rows(true));
    t.view(rows(false));
    t.advance(0.05);
    t.view(rows(true));
    t.settle();
    assert_eq!(shown(&mut t), ["a", "b", "c"]);
    let b = t.center("b");
    t.click(b);
    assert_eq!(t.messages(), [Msg::Click("b")]);
}

#[test]
fn without_disappear_an_element_goes_at_once() {
    let mut t = ui(200.0, 200.0);
    t.view(column().children((div().key("a").child(label("a")), div().key("b").child(label("b")))));
    t.view(column().child(div().key("a").child(label("a"))));
    assert_eq!(shown(&mut t), ["a"]);
}
