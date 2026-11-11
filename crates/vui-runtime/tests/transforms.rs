//! Transforms: an element scaled, turned or moved as it is drawn, in a layer of its own, without
//! moving in the layout; sprung with a transition.

mod common;

use common::*;
use vui_core::{Affine, Command, LayerCmd, Scene, Spring, Tone, pt};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Parent, Styled, column, div, label};

fn layers(scene: &Scene) -> Vec<&LayerCmd> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Layer(l) => Some(&**l),
            _ => None,
        })
        .collect()
}

fn card(scale: f32) -> impl vui_runtime::IntoElement<Msg> {
    column().padding(10.0).children((
        div().key("a").height(40.0).bg(Tone::Side),
        div().key("card").height(40.0).bg(Tone::Side).scale(scale).transition(Spring::SNAPPY).child(label("Inside")),
        div().key("b").height(40.0).bg(Tone::Side),
    ))
}

#[test]
fn an_untransformed_tree_draws_no_layers() {
    let mut t = ui(200.0, 200.0);
    t.view(card(1.0));
    assert!(layers(&t.scene()).is_empty());
}

#[test]
fn a_scaled_element_is_drawn_in_a_layer_and_keeps_its_place() {
    let mut t = ui(200.0, 200.0);
    t.view(card(1.0));
    let (a, card_rect, b) = (t.rect("a"), t.rect("card"), t.rect("b"));
    t.view(card(1.5));
    t.settle();
    assert_eq!((t.rect("a"), t.rect("card"), t.rect("b")), (a, card_rect, b), "layout ignores the transform");
    let scene = t.scene();
    let l = layers(&scene);
    assert_eq!(l.len(), 1);
    let centre = pt(card_rect.x + card_rect.w / 2.0, card_rect.y + card_rect.h / 2.0);
    let expected = Affine::scale(1.5, 1.5).around(centre);
    assert!((l[0].transform.a - expected.a).abs() < 1e-4 && (l[0].transform.e - expected.e).abs() < 1e-3);
    assert!(l[0].bounds.x <= card_rect.x && l[0].bounds.y <= card_rect.y, "the layer holds the card");
    assert!(texts(&scene).contains(&"Inside".to_string()), "its children are inside the layer");
}

#[test]
fn a_transition_springs_the_transform() {
    let mut t = ui(200.0, 200.0);
    t.view(card(1.0));
    t.view(card(2.0));
    t.advance(1.0 / 60.0);
    let a = layers(&t.scene())[0].transform.a;
    assert!(a > 1.0 && a < 2.0, "part way: {a}");
    t.settle();
    assert!((layers(&t.scene())[0].transform.a - 2.0).abs() < 1e-3);
    t.view(card(1.0));
    t.settle();
    assert!(layers(&t.scene()).is_empty(), "back to no layer at rest");
}

#[test]
fn rotate_translate_and_origin_compose() {
    let mut t = ui(200.0, 200.0);
    t.view(column().child(div().key("x").width(40.0).height(20.0).bg(Tone::Side).rotate(90.0).translate(5.0, 0.0).transform_origin(0.0, 0.0)));
    let r = t.rect("x");
    let scene = t.scene();
    let m = layers(&scene)[0].transform;
    // The top right corner turns about the top left one to straight below it, then moves right.
    let p = m.apply(pt(r.x + r.w, r.y));
    assert!((p.x - (r.x + 5.0)).abs() < 1e-3 && (p.y - (r.y + r.w)).abs() < 1e-3, "{p:?}");
}

#[test]
fn transforms_nest() {
    let mut t: TestUi<Msg> = ui(200.0, 200.0);
    t.view(column().child(div().key("outer").height(80.0).bg(Tone::Side).scale(0.5).child(div().key("inner").height(20.0).bg(Tone::Wash).rotate(10.0))));
    let scene = t.scene();
    let outer = layers(&scene);
    assert_eq!(outer.len(), 1);
    assert!(outer[0].commands.iter().any(|c| matches!(c, Command::Layer(_))), "the inner layer is inside the outer one");
}

mod keyframes {
    use super::*;
    use vui_core::{Easing, Keyframes, Repeat};
    use vui_runtime::Pose;

    fn pulsing(k: Keyframes<Pose>) -> impl vui_runtime::IntoElement<Msg> {
        column().child(div().key("dot").width(20.0).height(20.0).bg(Tone::Side).animate(k))
    }

    #[test]
    fn keyframes_move_the_element_and_finish() {
        let mut t = ui(100.0, 100.0);
        t.view(pulsing(Keyframes::new(1.0, Pose::REST).to(0.5, Pose::REST.scale(2.0), Easing::Linear).to(1.0, Pose::REST, Easing::Linear)));
        assert!(layers(&t.scene()).is_empty(), "at rest to start with");
        t.advance(0.5);
        let a = layers(&t.scene())[0].transform.a;
        assert!((a - 2.0).abs() < 0.05, "twice the size halfway: {a}");
        t.settle();
        assert!(!t.ui.needs_frame(), "a single run ends");
        assert!(layers(&t.scene()).is_empty());
    }

    #[test]
    fn looping_keyframes_keep_going_and_fade() {
        let mut t = ui(100.0, 100.0);
        let fade = Keyframes::new(1.0, Pose::REST).to(1.0, Pose::REST.opacity(0.0), Easing::Linear).repeat(Repeat::Forever);
        t.view(pulsing(fade));
        t.advance(10.5);
        assert!(t.ui.needs_frame(), "forever means forever");
        let fill = vui_runtime::testing::quads(&t.scene())[0].fill;
        assert!((fill.a - 0.5).abs() < 0.05, "half way through a run: {}", fill.a);
    }
}
