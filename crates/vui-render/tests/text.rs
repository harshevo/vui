//! Rendering tests for text: where the ink lands, clipping, colour, layering with shapes, scale, and a
//! golden sheet of the type scale in light and dark.

use vui_core::{Color, Fonts, Quad, Scene, TextAlign, TextStyle, Theme, pt, rect, size};
use vui_render::testing::render_scene;
use vui_render::{Image, assert_golden};

fn body() -> TextStyle {
    TextStyle::new(14.0, 21.0)
}

/// Bounding box of pixels that differ from white by more than `threshold`, as (x0, y0, x1, y1).
fn ink(img: &Image, threshold: u8) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..img.height {
        for x in 0..img.width {
            let p = img.pixel(x, y);
            if p[0].abs_diff(255).max(p[1].abs_diff(255)).max(p[2].abs_diff(255)) > threshold {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    b
}

fn count_ink(img: &Image, region: (u32, u32, u32, u32), threshold: u8) -> usize {
    let mut n = 0;
    for y in region.1..region.3.min(img.height) {
        for x in region.0..region.2.min(img.width) {
            let p = img.pixel(x, y);
            if p[0].abs_diff(255).max(p[1].abs_diff(255)).max(p[2].abs_diff(255)) > threshold {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn text_lands_inside_its_layout_box() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("Hello world", &body(), None);
    let mut s = Scene::new(size(160.0, 60.0), 1.0).with_clear(Color::WHITE);
    s.text(layout.clone(), pt(20.0, 10.0), Color::BLACK, None);
    let img = render_scene(&s, &fonts);
    let (x0, y0, x1, y1) = ink(&img, 40).expect("some ink");
    let sz = layout.size();
    assert!(x0 >= 19 && x1 <= (20.0 + sz.w) as u32 + 1, "ink x {x0}..{x1} vs box 20..{}", 20.0 + sz.w);
    assert!(y0 >= 10 && y1 <= (10.0 + sz.h) as u32, "ink y {y0}..{y1} vs box 10..{}", 10.0 + sz.h);
    assert!(count_ink(&img, (0, 0, 160, 60), 40) > 80, "the glyphs have real coverage");
    // ascenders and descenders stay inside the line box, with the baseline near 80% of the height
    assert!(y1 - y0 >= 9, "x-height plus ascender is at least 9 px at 14 px");
}

#[test]
fn text_is_cut_by_its_clip() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("Hello world", &body(), None);
    let mut s = Scene::new(size(160.0, 60.0), 1.0).with_clear(Color::WHITE);
    s.text(layout, pt(20.0, 10.0), Color::BLACK, Some(rect(0.0, 0.0, 50.0, 60.0)));
    let img = render_scene(&s, &fonts);
    assert!(count_ink(&img, (0, 0, 51, 60), 40) > 20, "the left part is drawn");
    assert_eq!(count_ink(&img, (52, 0, 160, 60), 8), 0, "nothing past the clip");
}

#[test]
fn text_takes_the_requested_colour() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("HHHH", &body().weight(600), None);
    let mut s = Scene::new(size(80.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.text(layout, pt(8.0, 8.0), Color::rgb(1.0, 0.0, 0.0), None);
    let img = render_scene(&s, &fonts);
    let mut strongest = [255u8; 4];
    for y in 0..img.height {
        for x in 0..img.width {
            let p = img.pixel(x, y);
            if p[1] < strongest[1] {
                strongest = p;
            }
        }
    }
    assert!(strongest[0] > 230 && strongest[1] < 40 && strongest[2] < 40, "stems should reach pure red, got {strongest:?}");
}

#[test]
fn text_and_shapes_layer_in_painter_order() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("Hello world", &body().weight(600), None);
    let cover = Quad::new(rect(0.0, 0.0, 160.0, 60.0)).fill(Color::WHITE);

    let mut under = Scene::new(size(160.0, 60.0), 1.0).with_clear(Color::WHITE);
    under.text(layout.clone(), pt(20.0, 10.0), Color::BLACK, None);
    under.quad(cover.clone());
    assert_eq!(count_ink(&render_scene(&under, &fonts), (0, 0, 160, 60), 8), 0, "a later opaque quad hides earlier text");

    let mut over = Scene::new(size(160.0, 60.0), 1.0).with_clear(Color::WHITE);
    over.quad(cover);
    over.text(layout, pt(20.0, 10.0), Color::BLACK, None);
    assert!(count_ink(&render_scene(&over, &fonts), (0, 0, 160, 60), 40) > 80, "later text is drawn over an earlier quad");
}

#[test]
fn text_on_a_filled_button_keeps_its_colour() {
    let fonts = Fonts::bundled_only();
    let t = Theme::light().palette;
    let layout = fonts.layout_cached("Convert", &body().weight(500), None);
    let mut s = Scene::new(size(120.0, 48.0), 1.0).with_clear(t.bg);
    s.quad(Quad::new(rect(10.0, 8.0, 100.0, 32.0)).fill(t.accent).radii(8.0));
    s.text(layout, pt(34.0, 13.0), t.on_accent, None);
    let img = render_scene(&s, &fonts);
    let mut brightest = [0u8; 4];
    for y in 8..40 {
        for x in 10..110 {
            let p = img.pixel(x, y);
            if p[0] as u32 + p[1] as u32 > brightest[0] as u32 + brightest[1] as u32 {
                brightest = p;
            }
        }
    }
    assert!(brightest[0] > 220 && brightest[1] > 220, "white label on the accent, got {brightest:?}");
}

#[test]
fn scale_two_doubles_the_ink() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("Hello world", &body(), None);
    let measure = |scale: f32| {
        let mut s = Scene::new(size(160.0, 60.0), scale).with_clear(Color::WHITE);
        s.text(layout.clone(), pt(20.0, 10.0), Color::BLACK, None);
        let img = render_scene(&s, &fonts);
        let (x0, _, x1, _) = ink(&img, 40).unwrap();
        (x1 - x0) as f32 / scale
    };
    let (w1, w2) = (measure(1.0), measure(2.0));
    assert!((w1 - w2).abs() < 2.0, "text is the same logical width at either scale: {w1} vs {w2}");
}

#[test]
fn many_alternating_runs_all_draw() {
    let fonts = Fonts::bundled_only();
    let layout = fonts.layout_cached("Row", &body().weight(600), None);
    let mut s = Scene::new(size(120.0, 400.0), 1.0).with_clear(Color::WHITE);
    for i in 0..16 {
        let y = i as f32 * 24.0 + 2.0;
        s.quad(Quad::new(rect(4.0, y, 112.0, 20.0)).fill(Color::hex(0xf0f0f0)).radii(6.0));
        s.text(layout.clone(), pt(10.0, y), Color::BLACK, None);
    }
    let img = render_scene(&s, &fonts);
    for i in 0..16u32 {
        let y = i * 24 + 2;
        assert!(count_ink(&img, (8, y, 50, y + 20), 100) > 15, "row {i} has text on its quad");
    }
}

#[test]
fn centred_and_end_aligned_text_move_inside_the_box() {
    let fonts = Fonts::bundled_only();
    let width = 200.0;
    let place = |align: TextAlign| {
        let l = fonts.layout_cached("Hi", &body().align(align), Some(width));
        let mut s = Scene::new(size(220.0, 40.0), 1.0).with_clear(Color::WHITE);
        s.text(l, pt(10.0, 8.0), Color::BLACK, None);
        let img = render_scene(&s, &fonts);
        ink(&img, 40).map(|(x0, _, x1, _)| (x0, x1)).unwrap()
    };
    let (start, centre, end) = (place(TextAlign::Start), place(TextAlign::Center), place(TextAlign::End));
    assert!(start.0 < 20);
    assert!((centre.0 as f32 - 105.0).abs() < 12.0, "centred near the middle: {centre:?}");
    assert!(end.1 > 195 && end.1 <= 211, "end-aligned against the right edge: {end:?}");
}

/// The type scale on light and dark surfaces, a wrapped paragraph, an ellipsized one and alignment.
#[test]
fn text_sheet() {
    let fonts = Fonts::bundled_only();
    let light = Theme::light();
    let dark = Theme::dark();
    let (lp, dp) = (&light.palette, &dark.palette);
    let ty = &light.types;
    let mut s = Scene::new(size(520.0, 500.0), 2.0).with_clear(lp.bg);

    let put = |s: &mut Scene, text: &str, style: TextStyle, at: (f32, f32), color: Color, width: Option<f32>| {
        let l = fonts.layout_cached(text, &style, width);
        s.text(l.clone(), pt(at.0, at.1), color, None);
        l
    };

    put(&mut s, "Allow three things", TextStyle::from_token(ty.title), (16.0, 14.0), lp.text, None);
    put(&mut s, "Hold the key, speak, and it happens.", TextStyle::from_token(ty.body), (16.0, 52.0), lp.text_2, None);
    put(&mut s, "Invoice-0423.docx", TextStyle::from_token(ty.body).weight(500), (16.0, 84.0), lp.text, None);
    put(&mut s, "Downloads", TextStyle::from_token(ty.small), (16.0, 105.0), lp.text_3, None);
    put(&mut s, "10:42", TextStyle::from_token(ty.mono).mono(), (440.0, 84.0), lp.text_3, None);
    put(&mut s, "TODAY", TextStyle::from_token(ty.small).weight(500).letter_spacing(0.08), (440.0, 108.0), lp.text_3, None);

    let para = "Each permission row ticks when macOS reports the grant, and Continue stays grey until the required ones are done.";
    for (i, lines) in [None, Some(3usize)].into_iter().enumerate() {
        let style = lines.map_or(TextStyle::from_token(ty.body), |n| TextStyle::from_token(ty.body).max_lines(n));
        let x = 16.0 + i as f32 * 252.0;
        let l = fonts.layout_cached(para, &style, Some(220.0));
        let h = l.size().h;
        s.quad(Quad::new(rect(x, 136.0, 236.0, h + 24.0)).fill(lp.bg).radii(light.radii.card).smooth(0.6).border(1.0, lp.line));
        s.text(l, pt(x + 8.0, 148.0), lp.text_2, None);
    }

    s.quad(Quad::new(rect(16.0, 276.0, 488.0, 32.0)).fill(lp.wash).radii(8.0).smooth(0.6));
    for (label, align) in [("Start", TextAlign::Start), ("Center", TextAlign::Center), ("End", TextAlign::End)] {
        put(&mut s, label, TextStyle::from_token(ty.body).align(align), (24.0, 282.0), lp.text, Some(472.0));
    }

    // the same on dark, with a primary button
    s.quad(Quad::new(rect(0.0, 330.0, 520.0, 170.0)).fill(dp.bg));
    put(&mut s, "Allow three things", TextStyle::from_token(ty.title), (16.0, 344.0), dp.text, None);
    put(&mut s, "Hold the key, speak, and it happens.", TextStyle::from_token(ty.body), (16.0, 382.0), dp.text_2, None);
    s.quad(Quad::new(rect(16.0, 420.0, 96.0, 32.0)).fill(dp.accent).radii(8.0).smooth(0.6));
    put(&mut s, "Convert", TextStyle::from_token(ty.body).weight(500).align(TextAlign::Center), (16.0, 425.5), dp.on_accent, Some(96.0));
    s.quad(Quad::new(rect(124.0, 420.0, 96.0, 32.0)).radii(8.0).smooth(0.6).border(1.0, dp.line));
    put(&mut s, "Cancel", TextStyle::from_token(ty.body).weight(500).align(TextAlign::Center), (124.0, 425.5), dp.text, Some(96.0));
    put(&mut s, "Sees Preview", TextStyle::from_token(ty.small), (16.0, 466.0), dp.text_3, None);

    let img = render_scene(&s, &fonts);
    assert_golden!("text_sheet", img);
}
