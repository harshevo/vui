//! Elastic: an element hung on one side slides in across it on a spring, a little past its place and
//! back, bent by its motion; dropped from the view, it slides out before it goes.

mod common;

use common::*;
use vui_core::{Command, Edge, Elastic, Preferences, Scene, Spring, Tone};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Parent, Styled, div, label, row};

const WIDE: f32 = 120.0;

fn bent(scene: &Scene) -> Option<Elastic> {
    scene.commands.iter().find_map(|c| match c {
        Command::Layer(l) => l.elastic,
        _ => None,
    })
}

/// A page, and at its right the room for a panel, which grows on the same spring as the panel
/// slides in.
fn desk(open: bool) -> impl vui_runtime::IntoElement<Msg> {
    let panel = div().key("panel").absolute().top(0.0).bottom(0.0).right(0.0).width(WIDE).bg(Tone::Side).elastic(Edge::Right).on_click(Msg::Click("panel")).child(label("Panel"));
    row().fill().children((
        div().key("page").grow(1.0).fill_height(),
        div().key("room").fill_height().width(if open { WIDE } else { 0.0 }).transition(if open { Spring::ELASTIC } else { Spring::ELASTIC_AWAY }).child(open.then_some(panel)),
    ))
}

fn shown(t: &mut TestUi<Msg>) -> bool {
    texts(&t.scene()).contains(&"Panel".to_string())
}

#[test]
fn it_slides_in_past_its_place_and_back() {
    let mut t = ui(400.0, 200.0);
    t.view(desk(false));
    t.view(desk(true));
    assert!(!shown(&mut t), "it starts all the way out");
    t.advance(1.0 / 60.0);
    let first = bent(&t.scene()).expect("bent while it moves");
    assert!(first.out > 0.0 && first.out < 1.0 && first.edge == Edge::Right);
    assert!(first.bow < 0.0, "coming in, its far side's middle lags behind its corners");
    let mut least = first.out;
    for _ in 0..90 {
        t.advance(1.0 / 120.0);
        if let Some(e) = bent(&t.scene()) {
            least = least.min(e.out);
        }
    }
    assert!(least < -0.05, "it went past its place: {least}");
    t.settle();
    let scene = t.scene();
    assert!(bent(&scene).is_none() && texts(&scene).contains(&"Panel".to_string()), "at rest it is drawn as it is");
    let c = t.center("panel");
    t.click(c);
    assert_eq!(t.messages(), [Msg::Click("panel")]);
}

#[test]
fn the_room_beside_it_keeps_pace_with_its_far_side() {
    let mut t = ui(400.0, 200.0);
    t.view(desk(false));
    t.view(desk(true));
    for open in [true, false] {
        if !open {
            t.settle();
            t.view(desk(false));
        }
        for _ in 0..40 {
            t.advance(1.0 / 120.0);
            let Some(e) = bent(&t.scene()) else { continue };
            let room = t.rect("room");
            let far = e.from.x + e.out * e.from.w;
            assert!((far - room.x).abs() < 0.75, "far side {far} and room {} part ({}, out {})", room.x, if open { "in" } else { "out" }, e.out);
        }
    }
}

#[test]
fn dropped_it_slides_out_then_goes() {
    let mut t = ui(400.0, 200.0);
    t.view(desk(true));
    t.settle();
    let c = t.center("panel");
    t.view(desk(false));
    assert!(shown(&mut t), "still drawn as it starts out");
    t.advance(0.05);
    let e = bent(&t.scene()).expect("bent while it moves");
    assert!(e.out > 0.0 && e.bow > 0.0, "going out its far side's middle lags the other way: {e:?}");
    t.click(c);
    assert!(t.messages().is_empty(), "it takes no clicks while it goes");
    t.advance(0.35);
    assert!(!shown(&mut t), "gone in well under half a second");
    t.settle();
    assert_eq!(t.rect("room").w, 0.0, "and its room with it, never less than nothing");
}

#[test]
fn brought_back_while_going_it_slides_back() {
    let mut t = ui(400.0, 200.0);
    t.view(desk(true));
    t.settle();
    t.view(desk(false));
    t.advance(0.05);
    t.view(desk(true));
    t.settle();
    assert!(shown(&mut t) && bent(&t.scene()).is_none());
    let c = t.center("panel");
    t.click(c);
    assert_eq!(t.messages(), [Msg::Click("panel")]);
}

#[test]
fn a_size_springing_past_nothing_stays_at_nothing() {
    let mut t = ui(400.0, 200.0);
    let room = |w: f32| row().fill().child(div().key("room").fill_height().width(w).transition(Spring::ELASTIC));
    t.view(room(WIDE));
    t.view(room(0.0));
    for _ in 0..60 {
        t.advance(1.0 / 120.0);
        assert!(t.rect("room").w >= 0.0);
    }
}

#[test]
fn with_reduced_motion_it_is_there_and_gone_at_once() {
    let mut t = ui(400.0, 200.0);
    t.ui.set_preferences(Preferences { reduce_motion: true, ..Preferences::DEFAULT });
    t.view(desk(false));
    t.view(desk(true));
    t.advance(1.0 / 60.0);
    assert!(shown(&mut t) && bent(&t.scene()).is_none());
    t.view(desk(false));
    t.advance(1.0 / 60.0);
    assert!(!shown(&mut t));
}
