//! Accessibility settings: reduced motion cuts springs and keyframes short, higher contrast and
//! larger text adapt the theme.

mod common;

use common::*;
use vui_core::{Easing, Keyframes, Preferences, Repeat, Spring, Tone};
use vui_runtime::testing::{TestUi, quads};
use vui_runtime::{Builder, Parent, Pose, Styled, column, div, label};

fn reduced() -> Preferences {
    Preferences { reduce_motion: true, ..Preferences::DEFAULT }
}

#[test]
fn reduced_motion_jumps_transitions_to_their_end() {
    let mut t = ui(200.0, 100.0);
    t.ui.set_preferences(reduced());
    let boxed = |w: f32| column().child(div().key("box").width(w).height(20.0).bg(Tone::Side).transition(Spring::SMOOTH));
    t.view(boxed(50.0));
    t.view(boxed(150.0));
    t.advance(1.0 / 60.0);
    assert_eq!(t.rect("box").w, 150.0, "one frame and it is there");
    assert!(!t.ui.needs_frame());
}

#[test]
fn reduced_motion_holds_keyframes_at_their_first_pose() {
    let mut t: TestUi<Msg> = ui(100.0, 100.0);
    t.ui.set_preferences(reduced());
    let fade = Keyframes::new(1.0, Pose::REST).to(1.0, Pose::REST.opacity(0.0), Easing::Linear).repeat(Repeat::Forever);
    t.view(column().child(div().width(20.0).height(20.0).bg(Tone::Side).animate(fade)));
    t.advance(0.5);
    assert_eq!(quads(&t.scene())[0].fill.a, 1.0);
    assert!(!t.ui.needs_frame(), "nothing keeps the window busy");
}

#[test]
fn larger_text_and_contrast_adapt_the_theme_and_back() {
    let mut t: TestUi<Msg> = ui(300.0, 100.0);
    t.view(column().child(label("Hello").key("l")));
    let (h, line) = (t.rect("l").h, t.ui.theme().palette.line);
    t.ui.set_preferences(Preferences { text_scale: 1.5, high_contrast: true, ..Preferences::DEFAULT });
    assert!(t.rect("l").h > h * 1.3, "the label grows with the text");
    assert_ne!(t.ui.theme().palette.line, line, "lines are stronger");
    t.ui.set_preferences(Preferences::DEFAULT);
    assert_eq!(t.rect("l").h, h);
    assert_eq!(t.ui.theme().palette.line, line, "the designed theme is kept underneath");
}
