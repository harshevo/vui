//! Rendering tests for the rounded-rectangle pipeline: exact pixel assertions on small scenes, plus a
//! golden image of a sheet of shapes to read by eye.

use vui_core::{Color, Corners, Fonts, Quad, Scene, Theme, rect, size};
use vui_render::testing::{render_scene, shared_gpu};
use vui_render::{Image, assert_golden};

fn render(scene: &Scene) -> Image {
    render_scene(scene, &Fonts::bundled_only())
}

const RED: Color = Color::rgb(1.0, 0.0, 0.0);

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= tol)
}

#[test]
fn the_gpu_is_a_real_one() {
    let info = shared_gpu().info();
    eprintln!("rendering on {} ({:?})", info.name, info.backend);
    assert_ne!(info.device_type, wgpu::DeviceType::Cpu, "software rendering would make golden images unreliable");
}

#[test]
fn a_filled_rect_covers_exactly_its_pixels() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(10.0, 10.0, 20.0, 20.0)).fill(RED));
    let img = render(&s);
    assert_eq!(img.pixel(20, 20), [255, 0, 0, 255]);
    assert_eq!(img.pixel(10, 10), [255, 0, 0, 255], "top-left pixel is inside");
    assert_eq!(img.pixel(29, 29), [255, 0, 0, 255], "bottom-right pixel is inside");
    assert_eq!(img.pixel(9, 20), [255, 255, 255, 255], "one pixel left of the rect is untouched");
    assert_eq!(img.pixel(30, 20), [255, 255, 255, 255], "one pixel right of the rect is untouched");
    assert_eq!(img.pixel(5, 5), [255, 255, 255, 255]);
}

#[test]
fn a_half_pixel_edge_is_half_covered() {
    let mut s = Scene::new(size(20.0, 10.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(5.5, 0.0, 10.0, 10.0)).fill(Color::BLACK));
    let img = render(&s);
    let edge = img.pixel(5, 5)[0];
    assert!((120..=136).contains(&edge), "expected ~50% grey on the edge pixel, got {edge}");
    assert_eq!(img.pixel(8, 5)[0], 0);
}

#[test]
fn a_circle_has_clear_corners_and_a_solid_middle() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 40.0, 40.0)).fill(RED).radii(20.0));
    let img = render(&s);
    assert_eq!(img.pixel(0, 0), [255, 255, 255, 255], "corner outside the circle");
    assert_eq!(img.pixel(39, 39), [255, 255, 255, 255]);
    assert_eq!(img.pixel(20, 20), [255, 0, 0, 255]);
    assert_eq!(img.pixel(20, 1), [255, 0, 0, 255], "top of the circle is inside");
    // the circle's edge crosses the diagonal near x = 5.86, so this pixel is only partly covered
    let p = img.pixel(5, 6);
    assert!(p[1] > 0 && p[1] < 255, "edge pixel should be partly covered, got {p:?}");
}

#[test]
fn radii_larger_than_the_box_make_a_pill() {
    let mut s = Scene::new(size(60.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 60.0, 20.0)).fill(RED).radii(9999.0));
    let img = render(&s);
    assert_eq!(img.pixel(0, 0), [255, 255, 255, 255]);
    assert_eq!(img.pixel(30, 10), [255, 0, 0, 255]);
    assert_eq!(img.pixel(2, 10), [255, 0, 0, 255], "the straight run of the pill's left end");
}

#[test]
fn only_the_requested_corners_are_rounded() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 40.0, 40.0)).fill(RED).radii(Corners::bottom(16.0)));
    let img = render(&s);
    assert_eq!(img.pixel(0, 0), [255, 0, 0, 255], "top-left stays square");
    assert_eq!(img.pixel(39, 0), [255, 0, 0, 255], "top-right stays square");
    assert_eq!(img.pixel(0, 39), [255, 255, 255, 255], "bottom-left is rounded away");
    assert_eq!(img.pixel(39, 39), [255, 255, 255, 255], "bottom-right is rounded away");
}

#[test]
fn the_border_sits_inside_the_rect() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::rgb(0.8, 0.8, 0.8));
    s.quad(Quad::new(rect(10.0, 10.0, 20.0, 20.0)).fill(Color::WHITE).border(2.0, Color::BLACK));
    let img = render(&s);
    assert_eq!(img.pixel(10, 20), [0, 0, 0, 255], "outer border pixel");
    assert_eq!(img.pixel(11, 20), [0, 0, 0, 255], "second border pixel");
    assert_eq!(img.pixel(12, 20), [255, 255, 255, 255], "first fill pixel");
    assert_eq!(img.pixel(20, 20), [255, 255, 255, 255]);
    assert_eq!(img.pixel(9, 20), [204, 204, 204, 255], "the border does not grow the box");
}

#[test]
fn a_border_only_quad_leaves_the_middle_open() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(5.0, 5.0, 30.0, 30.0)).radii(8.0).border(1.0, RED));
    let img = render(&s);
    assert_eq!(img.pixel(20, 20), [255, 255, 255, 255], "no fill, so the middle shows the background");
    assert_eq!(img.pixel(5, 20), [255, 0, 0, 255], "the 1 px edge is solid");
}

#[test]
fn translucent_fills_blend_in_encoded_space() {
    let mut s = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 20.0, 20.0)).fill(Color::BLACK.with_alpha(0.5)));
    let img = render(&s);
    // 50% black over white in encoded space is 128, not the 188 linear light would give.
    assert!(near(img.pixel(10, 10), [128, 128, 128, 255], 1), "{:?}", img.pixel(10, 10));
}

#[test]
fn later_quads_paint_over_earlier_ones() {
    let mut s = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 20.0, 20.0)).fill(RED));
    s.quad(Quad::new(rect(5.0, 5.0, 10.0, 10.0)).fill(Color::BLACK));
    let img = render(&s);
    assert_eq!(img.pixel(2, 2), [255, 0, 0, 255]);
    assert_eq!(img.pixel(10, 10), [0, 0, 0, 255]);
}

#[test]
fn a_clip_rect_cuts_the_quad() {
    let mut s = Scene::new(size(40.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 40.0, 20.0)).fill(RED).clip(Some(rect(10.0, 0.0, 10.0, 20.0))));
    let img = render(&s);
    assert_eq!(img.pixel(5, 10), [255, 255, 255, 255], "left of the clip");
    assert_eq!(img.pixel(15, 10), [255, 0, 0, 255], "inside the clip");
    assert_eq!(img.pixel(25, 10), [255, 255, 255, 255], "right of the clip");
}

#[test]
fn the_scale_factor_multiplies_everything() {
    let mut s = Scene::new(size(20.0, 20.0), 2.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(5.0, 5.0, 10.0, 10.0)).fill(RED).border(1.0, Color::BLACK));
    let img = render(&s);
    assert_eq!((img.width, img.height), (40, 40));
    assert_eq!(img.pixel(10, 20), [0, 0, 0, 255], "the 1 px border is 2 device pixels wide");
    assert_eq!(img.pixel(11, 20), [0, 0, 0, 255]);
    assert_eq!(img.pixel(12, 20), [255, 0, 0, 255]);
    assert_eq!(img.pixel(9, 20), [255, 255, 255, 255]);
}

#[test]
fn a_transparent_clear_keeps_the_frame_see_through() {
    let mut s = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::TRANSPARENT);
    s.quad(Quad::new(rect(5.0, 5.0, 10.0, 10.0)).fill(RED));
    let img = render(&s);
    assert_eq!(img.pixel(1, 1)[3], 0);
    assert_eq!(img.pixel(10, 10), [255, 0, 0, 255]);
}

#[test]
fn many_quads_grow_the_instance_buffer() {
    let mut s = Scene::new(size(200.0, 100.0), 1.0).with_clear(Color::WHITE);
    for i in 0..3000 {
        let (x, y) = ((i % 100) as f32 * 2.0, (i / 100) as f32 * 2.0);
        s.quad(Quad::new(rect(x, y, 2.0, 2.0)).fill(if (i + i / 100) % 2 == 0 { Color::BLACK } else { Color::WHITE }));
    }
    let img = render(&s);
    assert_eq!(img.pixel(0, 0), [0, 0, 0, 255]);
    assert_eq!(img.pixel(2, 0), [255, 255, 255, 255]);
    assert_eq!(img.pixel(198, 58), if (99 + 29) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
}

/// A sheet of shapes in light and dark, for reading by eye and for regression.
#[test]
fn shapes_sheet() {
    let light = Theme::light().palette;
    let dark = Theme::dark().palette;
    let mut s = Scene::new(size(520.0, 300.0), 2.0).with_clear(light.bg);
    let mut y = 16.0;

    // radii from square to pill, filled with the accent
    let mut x = 16.0;
    for r in [0.0, 4.0, 8.0, 12.0, 16.0, 9999.0] {
        s.quad(Quad::new(rect(x, y, 64.0, 32.0)).fill(light.accent).radii(r).smooth(0.6));
        x += 76.0;
    }

    // bordered controls: field, secondary button, selected tile ring
    y += 48.0;
    s.quad(Quad::new(rect(16.0, y, 150.0, 32.0)).fill(light.field).radii(8.0).smooth(0.6).border(1.0, light.line));
    s.quad(Quad::new(rect(178.0, y, 96.0, 32.0)).fill(light.field).radii(8.0).smooth(0.6).border(2.0, light.accent));
    s.quad(Quad::new(rect(286.0, y, 96.0, 32.0)).fill(light.wash).radii(8.0).smooth(0.6));
    s.quad(Quad::new(rect(394.0, y, 32.0, 32.0)).fill(light.danger).radii(9999.0));
    s.quad(Quad::new(rect(438.0, y, 64.0, 32.0)).radii(16.0).smooth(0.6).border(1.0, light.line));

    // circular corners against continuous corners at a large radius
    y += 48.0;
    s.quad(Quad::new(rect(16.0, y, 96.0, 64.0)).fill(light.text).radii(24.0).smooth(0.0));
    s.quad(Quad::new(rect(124.0, y, 96.0, 64.0)).fill(light.text).radii(24.0).smooth(0.6));
    s.quad(Quad::new(rect(232.0, y, 96.0, 64.0)).fill(light.text).radii(24.0).smooth(1.0));

    // translucent overlap and a hairline ring
    s.quad(Quad::new(rect(348.0, y, 64.0, 64.0)).fill(Color::rgb(0.86, 0.15, 0.15).with_alpha(0.6)).radii(12.0));
    s.quad(Quad::new(rect(380.0, y + 16.0, 64.0, 64.0)).fill(Color::rgb(0.15, 0.39, 0.92).with_alpha(0.6)).radii(12.0));
    s.quad(Quad::new(rect(456.0, y, 48.0, 64.0)).fill(Color::hex(0xfde9cf)).radii(12.0).smooth(0.6).border(1.0, light.ring));

    // a dark panel with the same pieces
    y += 96.0;
    s.quad(Quad::new(rect(0.0, y - 8.0, 520.0, 300.0 - y + 8.0)).fill(dark.bg));
    s.quad(Quad::new(rect(16.0, y + 8.0, 150.0, 32.0)).fill(dark.field).radii(8.0).smooth(0.6).border(1.0, dark.line));
    s.quad(Quad::new(rect(178.0, y + 8.0, 96.0, 32.0)).fill(dark.accent).radii(8.0).smooth(0.6));
    s.quad(Quad::new(rect(286.0, y + 8.0, 96.0, 32.0)).fill(dark.wash).radii(8.0).smooth(0.6));
    s.quad(Quad::new(rect(394.0, y + 8.0, 108.0, 56.0)).fill(dark.side).radii(12.0).smooth(0.6).border(1.0, dark.line));

    // a clipped quad: only the middle third of this bar is drawn
    s.quad(Quad::new(rect(16.0, y + 56.0, 360.0, 16.0)).fill(dark.accent).radii(8.0).clip(Some(rect(136.0, y + 56.0, 120.0, 16.0))));

    let img = render(&s);
    assert_golden!("shapes_sheet", img);
}
