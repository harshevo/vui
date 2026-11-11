//! Sliders: dragging, clicking the track, the keys and assistive technology.

use vui_core::{Color, NamedKey, Quad, Scene, Theme, pt};
use vui_runtime::testing::{TestUi, quads};
use vui_runtime::{AccessAction, App, Builder, Cx, IntoElement, Parent, Role, Styled, column};
use vui_widgets::slider;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Volume(f64),
    Level(f64),
}

struct Mixer {
    volume: f64,
    level: f64,
    log: Vec<Msg>,
    locked: bool,
}

impl App for Mixer {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match &msg {
            Msg::Volume(v) => self.volume = *v,
            Msg::Level(v) => self.level = *v,
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(20.0).gap(20.0).children((
            slider(self.volume, 0.0, 100.0).key("vol").name("Volume").width(200.0).on_change(Msg::Volume).disabled(self.locked),
            slider(self.level, 0.0, 10.0).step(1.0).key("level").name("Level").width(200.0).on_change(Msg::Level),
        ))
    }
}

fn mixer() -> TestUi<Msg> {
    TestUi::with_app(260.0, 120.0, Mixer { volume: 50.0, level: 5.0, log: Vec::new(), locked: false })
}

fn volume(t: &TestUi<Msg>) -> f64 {
    t.app::<Mixer>().volume
}

fn log(t: &TestUi<Msg>) -> Vec<Msg> {
    t.app::<Mixer>().log.clone()
}

/// The window x of a slider's thumb centre for `fraction` of the way along a slider that starts at `x`
/// and is 200 wide: the centre runs between 8 points in from each end.
fn thumb_x(x: f32, fraction: f32) -> f32 {
    x + 8.0 + 184.0 * fraction
}

fn near(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01 && (a.a - b.a).abs() < 0.01
}

fn accent() -> Color {
    Theme::light().palette.accent
}

/// The thumb: the 16 point circle with the accent border.
fn thumb(scene: &Scene) -> Vec<&Quad> {
    quads(scene).into_iter().filter(|q| q.rect.w == 16.0 && q.border.is_some_and(|b| b.width == 2.0 && near(b.color, accent()))).collect()
}

#[test]
fn the_thumb_sits_where_the_value_says() {
    let mut t = mixer();
    let r = t.rect("vol");
    assert_eq!((r.w, r.h), (200.0, 20.0));
    let scene = t.scene();
    let thumbs = thumb(&scene);
    assert_eq!(thumbs.len(), 2);
    assert!((thumbs[0].rect.center().x - thumb_x(r.x, 0.5)).abs() < 0.6, "{:?}", thumbs[0].rect);
}

#[test]
fn the_part_before_the_thumb_is_filled_with_the_accent() {
    let mut t = mixer();
    let r = t.rect("vol");
    let scene = t.scene();
    let filled: Vec<_> = quads(&scene).into_iter().filter(|q| near(q.fill, accent()) && q.rect.h == 4.0 && q.rect.y < r.bottom() && q.rect.y >= r.y).collect();
    assert_eq!(filled.len(), 1);
    assert!((filled[0].rect.right() - thumb_x(r.x, 0.5)).abs() < 0.6);
}

#[test]
fn clicking_the_track_moves_the_value_there() {
    let mut t = mixer();
    let r = t.rect("vol");
    t.click(pt(thumb_x(r.x, 0.25), r.center().y));
    assert!((volume(&t) - 25.0).abs() < 0.2, "{}", volume(&t));
}

#[test]
fn grabbing_the_thumb_off_centre_does_not_make_it_jump() {
    let mut t = mixer();
    let r = t.rect("vol");
    t.press(pt(thumb_x(r.x, 0.5) + 5.0, r.center().y));
    assert!(log(&t).is_empty(), "nothing moved yet");
    t.move_to(pt(thumb_x(r.x, 0.5) + 5.0 + 46.0, r.center().y));
    assert!((volume(&t) - 75.0).abs() < 0.2, "{}", volume(&t));
    t.release(pt(thumb_x(r.x, 0.5) + 51.0, r.center().y));
}

#[test]
fn a_drag_goes_on_outside_the_slider_and_stops_at_the_ends() {
    let mut t = mixer();
    let r = t.rect("vol");
    t.press(pt(thumb_x(r.x, 0.5), r.center().y));
    t.move_to(pt(r.right() + 80.0, r.center().y + 90.0));
    assert_eq!(volume(&t), 100.0);
    t.move_to(pt(r.x - 80.0, r.center().y - 10.0));
    assert_eq!(volume(&t), 0.0);
    t.release(pt(r.x - 80.0, r.center().y));
    t.move_to(pt(r.right(), r.center().y));
    assert_eq!(volume(&t), 0.0, "after the release the slider no longer follows the pointer");
}

#[test]
fn a_stepped_slider_lands_on_whole_steps() {
    let mut t = mixer();
    let r = t.rect("level");
    // A third of the way along the run of 0 to 10 is 3.33: it lands on 3.
    t.click(pt(thumb_x(r.x, 1.0 / 3.0), r.center().y));
    assert_eq!(t.app::<Mixer>().level, 3.0);
    t.press(pt(thumb_x(r.x, 0.3), r.center().y));
    t.move_to(pt(thumb_x(r.x, 0.86), r.center().y));
    assert_eq!(t.app::<Mixer>().level, 9.0);
    t.release(pt(thumb_x(r.x, 0.86), r.center().y));
}

#[test]
fn values_are_only_sent_when_they_change() {
    let mut t = mixer();
    let r = t.rect("vol");
    t.press(pt(thumb_x(r.x, 0.5), r.center().y));
    t.move_to(pt(thumb_x(r.x, 0.5) + 0.01, r.center().y));
    t.release(pt(thumb_x(r.x, 0.5) + 0.01, r.center().y));
    assert!(log(&t).iter().all(|m| matches!(m, Msg::Volume(v) if (*v - 50.0).abs() < 0.1)));
}

#[test]
fn the_keys_move_it_by_a_hundredth_a_tenth_or_to_the_ends() {
    let mut t = mixer();
    t.tab();
    assert_eq!(t.ui.focused_key(), Some("vol".into()));
    t.key(NamedKey::ArrowRight);
    assert_eq!(volume(&t), 51.0);
    t.key(NamedKey::ArrowUp);
    assert_eq!(volume(&t), 52.0);
    t.key(NamedKey::ArrowLeft);
    t.key(NamedKey::ArrowDown);
    assert_eq!(volume(&t), 50.0);
    t.key(NamedKey::PageUp);
    assert_eq!(volume(&t), 60.0);
    t.key(NamedKey::PageDown);
    t.key(NamedKey::PageDown);
    assert_eq!(volume(&t), 40.0);
    t.key(NamedKey::End);
    assert_eq!(volume(&t), 100.0);
    let sent = log(&t).len();
    t.key(NamedKey::ArrowRight);
    assert_eq!(log(&t).len(), sent, "already at the end: nothing to send");
    t.key(NamedKey::Home);
    assert_eq!(volume(&t), 0.0);
}

#[test]
fn a_stepped_slider_moves_by_its_step() {
    let mut t = mixer();
    t.tab();
    t.tab();
    assert_eq!(t.ui.focused_key(), Some("level".into()));
    t.key(NamedKey::ArrowRight);
    assert_eq!(t.app::<Mixer>().level, 6.0);
    t.key(NamedKey::PageDown);
    assert_eq!(t.app::<Mixer>().level, 0.0, "ten steps down, stopping at the minimum");
}

#[test]
fn the_focus_ring_goes_round_the_thumb_and_only_for_the_keyboard() {
    let mut t = mixer();
    let r = t.rect("vol");
    let ring = |scene: &Scene| quads(scene).iter().any(|q| q.border.is_some_and(|b| b.width == 2.0 && near(b.color, accent())) && q.rect.w == 22.0);
    t.click(pt(thumb_x(r.x, 0.5), r.center().y));
    t.settle();
    assert!(!ring(&t.scene()), "no ring after a click");
    t.tab();
    t.settle();
    assert!(ring(&t.scene()));
}

#[test]
fn hovering_the_thumb_lights_a_halo() {
    let mut t = mixer();
    let r = t.rect("vol");
    let halos = |scene: &Scene| quads(scene).iter().filter(|q| q.border.is_none() && q.rect.w == 22.0 && q.fill.a > 0.0 && near(Color { a: 1.0, ..q.fill }, accent())).count();
    assert_eq!(halos(&t.scene()), 0);
    t.move_to(pt(thumb_x(r.x, 0.5), r.center().y));
    t.settle();
    assert_eq!(halos(&t.scene()), 1);
}

#[test]
fn a_disabled_slider_ignores_the_pointer() {
    let mut t = mixer();
    t.update_app(|m: &mut Mixer| m.locked = true);
    let r = t.rect("vol");
    t.click(pt(thumb_x(r.x, 0.9), r.center().y));
    assert!(log(&t).is_empty());
}

#[test]
fn a_slider_reads_as_a_slider_with_a_range() {
    let mut t = mixer();
    let tree = t.ui.access_tree();
    let n = tree.find("Volume").expect("named").clone();
    assert_eq!(n.role, Role::Slider);
    let range = n.range.expect("a range");
    assert_eq!((range.min, range.max, range.value), (0.0, 100.0, 50.0));
    assert_eq!(n.value.as_deref(), Some("50"));
    assert!(n.focusable);
}

#[test]
fn assistive_technology_can_nudge_and_set_it() {
    let mut t = mixer();
    let id = t.ui.access_tree().find("Volume").unwrap().id;
    t.ui.do_action(id, AccessAction::Increment);
    assert_eq!(volume(&t), 51.0);
    t.ui.do_action(id, AccessAction::Decrement);
    t.ui.do_action(id, AccessAction::Decrement);
    assert_eq!(volume(&t), 49.0);
    t.ui.do_action(id, AccessAction::SetNumeric(80.0));
    assert_eq!(volume(&t), 80.0);
    t.ui.do_action(id, AccessAction::SetNumeric(500.0));
    assert_eq!(volume(&t), 100.0, "clamped to the range");
}
