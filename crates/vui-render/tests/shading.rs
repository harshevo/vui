//! The shading pass: a sheet with every kind of shaded fill, kept as a golden frame, and the CPU
//! renderer drawing the same sheet to within a few levels of the GPU.

use vui_core::shading::{self, Gradient, Ramp};
use vui_core::{Color, Command, Fonts, Images, Resources, Scene, Theme, pt, rect, size};
use vui_render::Image;
use vui_render::assert_golden;
use vui_render::testing::{render_cpu, render_with};

const INDIGO: Color = Color::hex(0x4f5bd5);
const PINK: Color = Color::hex(0xec4899);
const ORANGE: Color = Color::hex(0xf97316);
const TEAL: Color = Color::hex(0x14b8a6);

fn sheet(scale: f32) -> Scene {
    let palette = Theme::light().palette;
    let mut scene = Scene::new(size(560.0, 300.0), scale);
    scene.clear = Some(Color::hex(0xf5f5f6));
    let cell = |i: usize| rect(16.0 + (i % 4) as f32 * 136.0, 16.0 + (i / 4) as f32 * 140.0, 120.0, 120.0);
    let mut push = |c| scene.push(Command::Custom(c));

    push(shading::fill(cell(0), 16.0, &Gradient::linear(135.0, [INDIGO, PINK, ORANGE]), &palette));
    push(shading::fill(cell(1), 60.0, &Gradient::radial([Color::WHITE, TEAL, INDIGO]).at(0.3, 0.3).radius(0.9), &palette));
    push(shading::fill(cell(2), 16.0, &Gradient::conic([INDIGO, PINK, ORANGE, INDIGO]), &palette));
    push(shading::aurora(cell(3), 16.0, &Ramp::even(&[Color::hex(0x0b1020), INDIGO, PINK, TEAL]), 12.0, 0.45, 0.04, 2.0));

    push(shading::fill(cell(4), 16.0, &Gradient::linear(90.0, [Color::WHITE, Color::WHITE]), &palette));
    push(shading::border(cell(4), 16.0, 3.0, &Gradient::conic([INDIGO, PINK, ORANGE, INDIGO]), &palette));
    push(shading::beam(cell(5), 16.0, 2.0, &Ramp::even(&[INDIGO.with_alpha(0.0), INDIGO, PINK]), 3.0, 0.35, 0.6));
    push(shading::fill(cell(6), 16.0, &Gradient::linear(0.0, [Color::hex(0x18181b), Color::hex(0x18181b)]), &palette));
    push(shading::spotlight(cell(6), 16.0, pt(40.0, 50.0), 140.0, INDIGO.with_alpha(0.7)));
    push(shading::fill(cell(7), 16.0, &Gradient::linear(45.0, [TEAL, INDIGO]), &palette));
    push(shading::grain(cell(7), 16.0, 0.08));
    scene
}

fn resources() -> Resources {
    Resources { fonts: Fonts::bundled_only(), images: Images::new() }
}

#[test]
fn every_shading_kind() {
    let res = resources();
    assert_golden!("shading_sheet", render_with(&sheet(1.0), &res));
}

#[test]
fn every_shading_kind_at_double_scale() {
    let res = resources();
    assert_golden!("shading_sheet_2x", render_with(&sheet(2.0), &res));
}

#[test]
fn the_cpu_draws_the_same_sheet() {
    let res = resources();
    if vui_render::testing::uses_cpu() {
        return; // the golden comparison above already ran on the CPU
    }
    let gpu: Image = render_with(&sheet(1.0), &res);
    let cpu = render_cpu(&sheet(1.0), &res);
    let diff = cpu.diff(&gpu, 3);
    eprintln!("{} of {} pixels over 3, max {}", diff.differing_pixels, diff.total_pixels, diff.max_channel_diff);
    assert!(!diff.size_mismatch);
    assert!(diff.differing_pixels <= diff.total_pixels / 500, "{diff:?}");
}
