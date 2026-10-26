mod common;

use common::*;
use vui_core::{Color, Tone, pt};
use vui_runtime::testing::{TestUi, quads};
use vui_runtime::{Align, Builder, Parent, Styled, column, div, row};

fn first_fill(t: &mut TestUi<Msg>) -> Color {
    let scene = t.scene();
    quads(&scene)[0].fill
}

fn hoverable() -> impl vui_runtime::IntoElement<Msg> {
    column().child(div().key("row").height(40.0).bg(Tone::Side).hover_bg(Tone::Hover).press_bg(Tone::Wash))
}

#[test]
fn hover_fades_in_and_out() {
    let mut t = ui(200.0, 100.0);
    t.view(hoverable());
    let p = t.ui.theme().palette.clone();
    assert_eq!(first_fill(&mut t), p.side);

    t.move_to(pt(10.0, 10.0));
    t.advance(1.0 / 60.0);
    let mid = first_fill(&mut t);
    assert!(mid.r < p.side.r && mid.r > p.hover.r, "part way between the two greys: {mid:?}");

    t.settle();
    assert_eq!(first_fill(&mut t), p.hover);

    t.leave();
    t.settle();
    assert_eq!(first_fill(&mut t), p.side);
}

#[test]
fn the_fade_neither_overshoots_nor_stalls() {
    let mut t = ui(200.0, 100.0);
    t.view(hoverable());
    let p = t.ui.theme().palette.clone();
    t.move_to(pt(10.0, 10.0));
    let mut last = first_fill(&mut t).r;
    for _ in 0..40 {
        t.advance(1.0 / 60.0);
        let r = first_fill(&mut t).r;
        assert!(r <= last + 1e-6 && r >= p.hover.r - 1e-6, "monotonic and inside the range: {last} -> {r}");
        last = r;
    }
    assert!((last - p.hover.r).abs() < 1e-4, "arrived within 40 frames: {last}");
}

#[test]
fn press_has_its_own_fill() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(div().key("row").height(40.0).bg(Tone::Side).press_bg(Tone::Accent).on_click(Msg::Click("row"))));
    let p = t.ui.theme().palette.clone();
    t.press(pt(10.0, 10.0));
    t.settle();
    assert_eq!(first_fill(&mut t), p.accent);
    t.release(pt(10.0, 10.0));
    t.settle();
    assert_eq!(first_fill(&mut t), p.side);
}

#[test]
fn an_idle_ui_asks_for_nothing() {
    let mut t = ui(200.0, 100.0);
    t.view(hoverable());
    t.scene();
    t.settle();
    assert!(!t.ui.needs_frame());
    assert_eq!(t.ui.next_wake(), None);
    assert!(!t.ui.needs_paint());
    t.move_to(pt(10.0, 10.0));
    assert!(t.ui.needs_frame(), "hovering starts a fade");
    assert!(t.ui.needs_paint());
}

#[test]
fn time_only_passes_when_ticked() {
    let mut t = ui(200.0, 100.0);
    assert_eq!(t.ui.time(), 0.0);
    t.advance(0.5);
    assert!((t.ui.time() - 0.5).abs() < 1e-4);
}

#[test]
fn a_size_change_glides_when_the_element_is_animated() {
    let mut t = ui(400.0, 100.0);
    let view = |w: f32| {
        column().child(row().align_items(Align::Start).children((
            div().key("a").width(w).height(20.0).animated(),
            div().key("b").width(10.0).height(20.0),
        )))
    };
    t.view(view(100.0));
    assert_eq!((t.rect("a").w, t.rect("b").x), (100.0, 100.0));

    t.view(view(200.0));
    assert_eq!(t.rect("a").w, 100.0, "no jump");
    t.advance(0.05);
    let (w, x) = (t.rect("a").w, t.rect("b").x);
    assert!(w > 100.0 && w < 200.0, "on its way: {w}");
    assert_eq!(x, w, "the neighbour moves with it");
    t.settle();
    assert_eq!((t.rect("a").w, t.rect("b").x), (200.0, 200.0));
    assert!(!t.ui.needs_frame());
}

#[test]
fn a_size_change_is_instant_without_a_transition() {
    let mut t = ui(400.0, 100.0);
    let view = |w: f32| column().child(row().align_items(Align::Start).child(div().key("a").width(w).height(20.0)));
    t.view(view(100.0));
    t.view(view(200.0));
    assert_eq!(t.rect("a").w, 200.0);
    assert!(!t.ui.needs_frame());
}

#[test]
fn a_retargeted_transition_keeps_its_momentum() {
    let mut t = ui(400.0, 100.0);
    let view = |w: f32| column().child(row().align_items(Align::Start).child(div().key("a").width(w).height(20.0).animated()));
    t.view(view(100.0));
    t.view(view(300.0));
    t.advance(0.06);
    let mid = t.rect("a").w;
    t.view(view(100.0));
    t.advance(1.0 / 60.0);
    let after = t.rect("a").w;
    assert!(after > mid - 20.0, "it was heading up, so it does not snap straight back: {mid} -> {after}");
    t.settle();
    assert_eq!(t.rect("a").w, 100.0);
}

#[test]
fn a_changed_fill_blends_when_the_element_is_animated() {
    let mut t = ui(200.0, 100.0);
    let view = |tone: Tone| column().child(div().key("a").height(40.0).bg(tone).animated());
    t.view(view(Tone::Wash));
    let p = t.ui.theme().palette.clone();
    t.view(view(Tone::Accent));
    assert_eq!(first_fill(&mut t), p.wash, "the change has not started to show");
    t.advance(0.05);
    let mid = first_fill(&mut t);
    assert!(mid.r < p.wash.r && mid.r > p.accent.r, "part way from grey to blue: {mid:?}");
    t.settle();
    assert_eq!(first_fill(&mut t), p.accent);
}

#[test]
fn a_fill_that_appears_from_nothing_does_not_flash_dark() {
    let mut t = ui(200.0, 100.0);
    let view = |fill: Option<Tone>| {
        let d = div().key("a").height(40.0).animated();
        column().child(match fill {
            Some(tone) => d.bg(tone),
            None => d,
        })
    };
    t.view(view(None));
    t.view(view(Some(Tone::Wash)));
    t.advance(0.03);
    let p = t.ui.theme().palette.clone();
    let mid = first_fill(&mut t);
    assert!(mid.a > 0.0 && mid.a < 1.0, "fading in: {mid:?}");
    assert!((mid.r - p.wash.r).abs() < 1e-4, "same colour, just less of it: {mid:?}");
}

#[test]
fn an_opacity_change_fades_when_the_element_is_animated() {
    let mut t = ui(200.0, 100.0);
    let view = |o: f32| column().child(div().key("a").height(40.0).bg(Tone::Wash).opacity(o).animated());
    t.view(view(1.0));
    t.view(view(0.0));
    t.advance(0.04);
    let a = first_fill(&mut t).a;
    assert!(a > 0.0 && a < 1.0, "{a}");
    t.settle();
    assert!(t.scene().commands.is_empty(), "fully faded out draws nothing");
}

#[test]
fn a_theme_switch_does_not_animate() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(div().key("a").height(40.0).bg(Tone::Side).animated()));
    t.ui.set_theme(vui_core::Theme::dark());
    assert_eq!(first_fill(&mut t), vui_core::Theme::dark().palette.side);
}
