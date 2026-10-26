//! The whole icon set on one sheet, to read by eye, kept as a golden image.

use vui_core::{Color, ImageCmd, Resources, Sampling, Scene, TextStyle, Theme, pt, rect, size};
use vui_icons::{SPINNER_FRAMES, Weight, set, spinner};
use vui_render::assert_golden;
use vui_render::testing::render_with;

const COLUMNS: usize = 8;
const CELL_W: f32 = 120.0;

/// Every icon at `big` points and at 16, with its name below, then the four weights and a spinner's
/// positions. `scale` is device pixels per point.
fn sheet(theme: &Theme, scale: f32, big: f32, weight: Weight) -> vui_render::Image {
    let res = Resources::bundled_only();
    let cell_h = big + 52.0;
    let rows = set::ALL.len().div_ceil(COLUMNS);
    let p = &theme.palette;
    let height = cell_h * (rows + 1) as f32 + 8.0;
    let mut scene = Scene::new(size(CELL_W * COLUMNS as f32, height), scale).with_clear(p.bg);
    let label = |scene: &mut Scene, text: &str, at: (f32, f32)| {
        let layout = res.fonts.layout_cached(text, &TextStyle::from_token(theme.types.small).weight(400), None);
        scene.text(layout, pt(at.0, at.1), p.text_3, None);
    };
    let draw = |scene: &mut Scene, id: vui_core::ImageId, px: u32, at: (f32, f32), color: Color| {
        let side = px as f32 / scale;
        scene.image(ImageCmd::new(id, rect(at.0, at.1, side, side)).mask(color).sampling(Sampling::Nearest));
    };
    let px = |points: f32| (points * scale).round() as u32;

    for (i, icon) in set::ALL.iter().enumerate() {
        let (x, y) = ((i % COLUMNS) as f32 * CELL_W, (i / COLUMNS) as f32 * cell_h);
        draw(&mut scene, icon.image(&res.images, px(big), weight), px(big), (x + 14.0, y + 10.0), p.text);
        let small = (x + 14.0 + big + 12.0, y + 10.0 + (big - 16.0) / 2.0);
        draw(&mut scene, icon.image(&res.images, px(16.0), weight), px(16.0), small, p.text_2);
        label(&mut scene, icon.name(), (x + 14.0, y + 12.0 + big + 8.0));
    }

    // The weights, then a spinner.
    let y = rows as f32 * cell_h;
    for (i, (w, name)) in [(Weight::Light, "light"), (Weight::Regular, "regular"), (Weight::Medium, "medium"), (Weight::Bold, "bold")]
        .into_iter()
        .enumerate()
    {
        let x = i as f32 * CELL_W;
        let id = set::SETTINGS.image(&res.images, px(big), w);
        draw(&mut scene, id, px(big), (x + 14.0, y + 10.0), p.text);
        label(&mut scene, name, (x + 14.0, y + 12.0 + big + 8.0));
    }
    for (i, frame) in [0, 10, 20, 30].into_iter().enumerate() {
        let x = (4 + i) as f32 * CELL_W;
        let id = spinner::image(&res.images, px(big), frame, weight);
        draw(&mut scene, id, px(big), (x + 14.0, y + 10.0), p.accent);
        label(&mut scene, &format!("spinner {frame}/{SPINNER_FRAMES}"), (x + 14.0, y + 12.0 + big + 8.0));
    }
    render_with(&scene, &res)
}

#[test]
fn the_icon_set_sheet_light() {
    assert_golden!("icons_sheet_light", sheet(&Theme::light(), 2.0, 28.0, Weight::Regular));
}

#[test]
fn the_icon_set_sheet_dark() {
    assert_golden!("icons_sheet_dark", sheet(&Theme::dark(), 2.0, 28.0, Weight::Regular));
}

/// Not a check: a large sheet to judge shapes by. `VUI_SHEET` says where to write it.
#[test]
#[ignore = "writes a large sheet for looking at"]
fn the_icon_set_sheet_large() {
    let img = sheet(&Theme::light(), 3.0, 56.0, Weight::Regular);
    let path = std::env::var("VUI_SHEET").unwrap_or_else(|_| "/tmp/icons_large.png".into());
    img.save_png(std::path::Path::new(&path)).unwrap();
}
