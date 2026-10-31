//! The built-in effects pass: shadows fall off softly and leave the surface clean, the shimmer band
//! is where its time says, and the glow ring sits on the edge. Plus a golden sheet to read by eye.

use vui_core::{Color, Command, Fonts, Quad, Scene, Shadow, effects, rect, size};
use vui_render::testing::render_scene;
use vui_render::{Image, assert_golden};

fn render(scene: &Scene) -> Image {
    render_scene(scene, &Fonts::bundled_only())
}

fn grey(img: &Image, x: u32, y: u32) -> u8 {
    img.pixel(x, y)[0]
}

#[test]
fn a_shadow_darkens_below_the_box_and_fades_with_distance() {
    let mut s = Scene::new(size(120.0, 120.0), 1.0).with_clear(Color::WHITE);
    let r = rect(30.0, 30.0, 60.0, 40.0);
    s.push(Command::Custom(effects::shadow(r, 8.0, Shadow { blur: 16.0, y: 6.0, spread: 0.0, alpha: 0.5 })));
    let img = render(&s);
    let near = grey(&img, 60, 74);
    let far = grey(&img, 60, 90);
    assert!(near < 235, "just below the box is shaded: {near}");
    assert!(far > near, "further away is lighter: {far} vs {near}");
    assert_eq!(grey(&img, 60, 118), 255, "past its reach the page is untouched");
    assert_eq!(grey(&img, 60, 50), 255, "nothing is drawn under the surface itself");
}

#[test]
fn the_shimmer_band_moves_with_time() {
    let band = |time: f32| {
        let mut s = Scene::new(size(200.0, 20.0), 1.0).with_clear(Color::BLACK);
        s.push(Command::Custom(effects::shimmer(rect(0.0, 0.0, 200.0, 20.0), 0.0, Color::WHITE, 2.0, time)));
        let img = render(&s);
        (0..200).max_by_key(|&x| grey(&img, x, 10)).unwrap()
    };
    let (early, later) = (band(0.5), band(0.9));
    assert!(later > early + 20, "the band moves right: {early} then {later}");
}

#[test]
fn the_glow_ring_is_brightest_on_the_edge() {
    let mut s = Scene::new(size(100.0, 100.0), 1.0).with_clear(Color::BLACK);
    s.push(Command::Custom(effects::glow(rect(30.0, 30.0, 40.0, 40.0), 6.0, Color::WHITE, 2.0, 6.0, 0.0, 0.0)));
    let img = render(&s);
    let edge = grey(&img, 50, 30);
    assert!(edge > 200, "on the edge: {edge}");
    assert!(grey(&img, 50, 24) < edge && grey(&img, 50, 24) > 0, "a soft glow outside");
    assert_eq!(grey(&img, 50, 50), 0, "the middle stays clear");
}

#[test]
fn effects_sheet() {
    let mut s = Scene::new(size(360.0, 200.0), 2.0).with_clear(Color::rgb(0.98, 0.98, 0.98));
    for (i, sh) in [Shadow::SOFT, Shadow::RAISED, Shadow::FLOATING].into_iter().enumerate() {
        let r = rect(24.0 + i as f32 * 112.0, 24.0, 88.0, 64.0);
        s.push(Command::Custom(effects::shadow(r, 12.0, sh)));
        s.quad(Quad::new(r).fill(Color::WHITE).radii(12.0));
    }
    let bar = rect(24.0, 124.0, 200.0, 36.0);
    s.quad(Quad::new(bar).fill(Color::rgb(0.9, 0.9, 0.9)).radii(10.0));
    s.push(Command::Custom(effects::shimmer(bar, 10.0, Color::rgba(1.0, 1.0, 1.0, 0.8), 2.0, 0.8)));
    let pill = rect(256.0, 124.0, 80.0, 36.0);
    s.quad(Quad::new(pill).fill(Color::WHITE).radii(18.0));
    s.push(Command::Custom(effects::glow(pill, 18.0, Color::rgb(0.2, 0.45, 1.0), 1.5, 8.0, 0.0, 0.0)));
    assert_golden!("effects_sheet", render(&s));
}
