//! The glass pass: it blurs what was drawn behind it and only there, tints it, pours one shape out
//! of another, and the GPU and the CPU draw it alike. Plus a golden sheet to read by eye.

use vui_core::glass::{self, pour};
use vui_core::{Color, Command, Fonts, Glass, Mode, Quad, Resources, Scene, rect, size};
use vui_render::testing::{render_cpu, render_scene, try_shared_gpu, uses_cpu};
use vui_render::{Image, assert_golden};

fn render(scene: &Scene) -> Image {
    render_scene(scene, &Fonts::bundled_only())
}

/// Black and white stripes, 8 points wide, under everything.
fn stripes(s: &mut Scene, w: f32, h: f32) {
    let mut x = 0.0;
    while x < w {
        s.quad(Quad::new(rect(x, 0.0, 8.0, h)).fill(Color::BLACK));
        x += 16.0;
    }
}

fn clear_glass(blur: f32) -> Glass {
    Glass { tint: Color::WHITE, opacity: 0.0, blur, saturation: 1.0, refraction: 0.0, dispersion: 0.0, rim: 0.0, specular: 0.0, grain: 0.0, floating: false }
}

fn grey(img: &Image, x: u32, y: u32) -> u8 {
    img.pixel(x, y)[0]
}

#[test]
fn glass_blurs_what_is_behind_it_and_nothing_else() {
    let mut s = Scene::new(size(160.0, 80.0), 1.0).with_clear(Color::WHITE);
    stripes(&mut s, 160.0, 80.0);
    s.push(Command::Custom(glass::glass(rect(40.0, 20.0, 80.0, 40.0), 12.0, &clear_glass(24.0))));
    let img = render(&s);
    let inside: Vec<u8> = (48..112).map(|x| grey(&img, x, 40)).collect();
    let (lo, hi) = (*inside.iter().min().unwrap(), *inside.iter().max().unwrap());
    assert!(hi - lo < 40, "under the glass the stripes melt into grey: {lo}..{hi}");
    assert!((90..170).contains(&lo), "and the grey is their mean: {lo}");
    assert_eq!(grey(&img, 4, 40), 0, "outside the glass a black stripe stays black");
    assert_eq!(grey(&img, 12, 40), 255, "and a white one white");
}

#[test]
fn the_tint_covers_as_much_as_its_opacity() {
    let mut s = Scene::new(size(60.0, 60.0), 1.0).with_clear(Color::BLACK);
    let g = Glass { tint: Color::WHITE, opacity: 0.5, ..clear_glass(0.0) };
    s.push(Command::Custom(glass::glass(rect(10.0, 10.0, 40.0, 40.0), 0.0, &g)));
    let v = grey(&render(&s), 30, 30);
    assert!((120..=136).contains(&v), "half white over black: {v}");
}

#[test]
fn a_pour_joins_two_boxes_with_a_neck() {
    let g = Glass { tint: Color::WHITE, opacity: 1.0, ..clear_glass(0.0) };
    let shot = |k: f32| {
        let mut s = Scene::new(size(100.0, 100.0), 1.0).with_clear(Color::BLACK);
        s.push(Command::Custom(pour(rect(30.0, 0.0, 40.0, 30.0), 10.0, rect(20.0, 40.0, 60.0, 50.0), 14.0, k, &g)));
        render(&s)
    };
    assert_eq!(grey(&shot(0.0), 50, 35), 0, "a hard union leaves the gap open");
    assert!(grey(&shot(30.0), 50, 35) > 200, "a liquid one fills it");
}

#[test]
fn the_gpu_and_the_cpu_draw_glass_alike() {
    if uses_cpu() || try_shared_gpu().is_none() {
        return;
    }
    let mut s = Scene::new(size(200.0, 120.0), 2.0).with_clear(Color::rgb(0.95, 0.9, 0.8));
    stripes(&mut s, 200.0, 120.0);
    s.quad(Quad::new(rect(20.0, 70.0, 60.0, 40.0)).fill(Color::rgb(0.1, 0.4, 0.9)).radii(8.0));
    s.push(Command::Custom(pour(rect(60.0, 10.0, 80.0, 24.0), 12.0, rect(40.0, 40.0, 120.0, 60.0), 16.0, 24.0, &Glass::for_mode(Mode::Dark))));
    let resources = Resources { fonts: Fonts::bundled_only(), images: Default::default() };
    let gpu = render(&s);
    let cpu = render_cpu(&s, &resources);
    let diff = gpu.diff(&cpu, 3);
    assert!(
        (diff.differing_pixels as f32) < diff.total_pixels as f32 * 0.002,
        "{} of {} pixels differ, by up to {}",
        diff.differing_pixels,
        diff.total_pixels,
        diff.max_channel_diff
    );
}

#[test]
fn glass_sheet() {
    let mut s = Scene::new(size(360.0, 200.0), 2.0).with_clear(Color::rgb(0.96, 0.94, 0.9));
    for (i, c) in [0xe8543d, 0x2f7de1, 0x36b37e, 0xf5a623, 0x8e44ad].into_iter().enumerate() {
        s.quad(Quad::new(rect(10.0 + i as f32 * 70.0, 20.0, 50.0, 160.0)).fill(Color::hex(c)).radii(25.0));
    }
    s.push(Command::Custom(glass::glass(rect(24.0, 40.0, 140.0, 120.0), 18.0, &Glass::for_mode(Mode::Light))));
    s.push(Command::Custom(pour(rect(220.0, 20.0, 90.0, 26.0), 13.0, rect(196.0, 60.0, 140.0, 100.0), 20.0, 28.0, &Glass::for_mode(Mode::Dark))));
    s.push(Command::Custom(glass::pill(rect(220.0, 170.0, 90.0, 22.0), &Glass::for_mode(Mode::Dark))));
    assert_golden!("glass_sheet", render(&s));
}
