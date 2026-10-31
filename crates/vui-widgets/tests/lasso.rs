//! The lasso: a loop sends its points, a click or a scribble cancels, and the stroke is drawn while it is.

use vui_core::{Point, pt};
use vui_runtime::testing::{TestUi, quads};
use vui_runtime::{Builder, Styled};
use vui_widgets::{lasso, lasso_bounds};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Circled(Vec<Point>),
    Cancelled,
}

fn surface() -> TestUi<Msg> {
    let mut t = TestUi::new(400.0, 300.0);
    t.view(lasso().key("lasso").width(400.0).height(300.0).on_done(Msg::Circled).on_cancel(|| Msg::Cancelled));
    t
}

#[test]
fn a_loop_sends_its_points_and_their_bounds_cover_it() {
    let mut t = surface();
    t.press(pt(100.0, 100.0));
    for i in 1..=24 {
        let a = i as f32 / 24.0 * std::f32::consts::TAU;
        t.move_to(pt(150.0 - 50.0 * a.cos(), 100.0 + 40.0 * a.sin()));
    }
    assert!(quads(&t.scene()).len() > 20, "the stroke shows while drawing");
    t.release(pt(100.0, 100.0));
    let messages = t.messages();
    let [Msg::Circled(points)] = &messages[..] else { panic!("{messages:?}") };
    let area = lasso_bounds(points);
    assert!((area.x - 100.0).abs() < 2.0 && (area.w - 100.0).abs() < 4.0 && (area.h - 80.0).abs() < 4.0, "{area:?}");
    assert!(quads(&t.scene()).is_empty(), "the stroke goes once it is sent");
}

#[test]
fn a_click_or_a_tiny_scribble_cancels() {
    let mut t = surface();
    t.click(pt(50.0, 50.0));
    t.press(pt(10.0, 10.0)).move_to(pt(14.0, 30.0)).release(pt(14.0, 30.0));
    assert_eq!(t.messages(), [Msg::Cancelled, Msg::Cancelled]);
}
