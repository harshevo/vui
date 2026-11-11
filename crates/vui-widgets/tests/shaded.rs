//! The shaded boxes: gradients from the element's style, the aurora, the border beam and the
//! spotlight. What they draw is checked as golden frames in vui-render; here, which commands they
//! send, where, and when they ask for frames.

use vui_core::shading::{self, Gradient};
use vui_core::{Command, CustomCmd, Scene, Tone, pt};
use vui_runtime::testing::TestUi;
use vui_runtime::{Align, Builder, IntoElement, Parent, Styled, column, div, label};
use vui_widgets::{aurora, border_beam, spotlight};

fn shaded(scene: &Scene) -> Vec<&CustomCmd> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Custom(c) if c.pass == shading::PASS => Some(c),
            _ => None,
        })
        .collect()
}

fn kind(c: &CustomCmd) -> f32 {
    c.data[0]
}

fn window(view: impl IntoElement<()>) -> TestUi<()> {
    let mut t = TestUi::new(320.0, 240.0);
    t.view(column().padding(20.0).align_items(Align::Start).child(view));
    t
}

#[test]
fn a_gradient_fills_the_element_under_its_children() {
    let mut t = window(div().key("g").width(100.0).height(40.0).rounded(8.0).gradient(Gradient::linear(90.0, [Tone::Accent, Tone::Purple])).child(label("Hi")));
    let scene = t.scene();
    let found = shaded(&scene);
    assert_eq!(found.len(), 1);
    assert_eq!(kind(found[0]), shading::LINEAR);
    assert_eq!(found[0].rect, t.rect("g"));
    let at = |pred: &dyn Fn(&Command) -> bool| scene.commands.iter().position(pred).unwrap();
    let text = at(&|c| matches!(c, Command::Text(_)));
    let fill = at(&|c| matches!(c, Command::Custom(_)));
    assert!(fill < text, "the label is drawn over the gradient");
}

#[test]
fn a_gradient_border_is_a_ring() {
    let mut t = window(div().width(100.0).height(40.0).gradient_border(2.0, Gradient::conic([Tone::Accent, Tone::Purple])));
    let scene = t.scene();
    let found = shaded(&scene);
    assert_eq!(found.len(), 1);
    assert_eq!(kind(found[0]), shading::CONIC);
    assert_eq!(*found[0].data.last().unwrap(), 2.0, "the ring width");
}

#[test]
fn an_aurora_moves_unless_held_still() {
    let mut t = window(aurora().key("a").width(200.0).height(100.0).child(label("Hello")));
    assert!(t.ui.needs_frame());
    let time = |t: &mut TestUi<()>| shaded(&t.scene())[0].data[31];
    let first = time(&mut t);
    t.advance(0.5);
    assert!(time(&mut t) > first);
    assert_eq!(kind(shaded(&t.scene())[0]), shading::AURORA);

    let mut t = window(aurora().still(3.0).width(200.0).height(100.0));
    t.settle();
    assert!(!t.ui.needs_frame());
    assert_eq!(time(&mut t), 3.0);
}

#[test]
fn a_border_beam_draws_over_its_children_and_fades_out_when_off() {
    let mut t = window(border_beam(true).width(200.0).height(60.0).child(label("Syncing")));
    let scene = t.scene();
    let beam = scene.commands.iter().position(|c| matches!(c, Command::Custom(c) if c.pass == shading::PASS)).unwrap();
    let text = scene.commands.iter().position(|c| matches!(c, Command::Text(_))).unwrap();
    assert!(beam > text, "the streak runs over the contents");
    assert!(t.ui.needs_frame());

    t.view(column().padding(20.0).align_items(Align::Start).child(border_beam(false).width(200.0).height(60.0).child(label("Syncing"))));
    t.settle();
    assert!(shaded(&t.scene()).is_empty());
    assert!(!t.ui.needs_frame());
}

#[test]
fn a_spotlight_follows_the_pointer_and_fades_when_it_leaves() {
    let mut t = window(spotlight().key("s").width(200.0).height(100.0).child(label("Team")));
    assert!(shaded(&t.scene()).is_empty(), "dark until the pointer comes");
    let r = t.rect("s");
    t.move_to(pt(r.x + 30.0, r.y + 40.0)).settle();
    let lit = shaded(&t.scene()).into_iter().find(|c| kind(c) == shading::SPOTLIGHT).cloned().expect("lit");
    assert_eq!((lit.data[6], lit.data[7]), (30.0, 40.0), "the light is where the pointer is");

    t.move_to(pt(r.x + 150.0, r.y + 10.0)).settle();
    let lit = shaded(&t.scene())[0].clone();
    assert_eq!((lit.data[6], lit.data[7]), (150.0, 10.0));

    t.leave().settle();
    assert!(shaded(&t.scene()).is_empty());
}

#[test]
fn a_pinned_spotlight_ignores_the_pointer() {
    let mut t = window(spotlight().at(10.0, 20.0).key("s").width(200.0).height(100.0));
    let r = t.rect("s");
    t.move_to(pt(r.x + 150.0, r.y + 80.0)).settle();
    let lit = shaded(&t.scene())[0].clone();
    assert_eq!((lit.data[6], lit.data[7]), (10.0, 20.0));
}

#[test]
fn hovering_a_gradient_button_tints_over_the_gradient() {
    let mut t = window(vui_widgets::button::<()>("Upgrade").key("b").primary().gradient(Gradient::linear(90.0, [Tone::Accent, Tone::Purple])));
    let order = |t: &mut TestUi<()>| {
        let scene = t.scene();
        let fill = scene.commands.iter().position(|c| matches!(c, Command::Custom(c) if c.pass == shading::PASS)).unwrap();
        scene.commands[fill + 1..].iter().filter(|c| matches!(c, Command::Quad(q) if q.fill.a > 0.0)).count()
    };
    let before = order(&mut t);
    let c = t.center("b");
    t.move_to(c).settle();
    assert_eq!(order(&mut t), before + 1, "the hover tint lands on top of the gradient");
}
