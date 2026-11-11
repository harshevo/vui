//! The golden sheet: photos, icons and stickers in every mode, side by side.

use super::*;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A landscape to look at: sky, sun, two ridges. `seed` moves the sun and the hills.
pub(crate) fn photo(w: u32, h: u32, seed: f32) -> ImageData {
    let aspect = w as f32 / h as f32;
    ImageData::from_fn(w, h, |x, y| {
        let u = (x as f32 + 0.5) / w as f32;
        let v = (y as f32 + 0.5) / h as f32;
        let mut c = [lerp(0.16, 0.97, v.powf(1.6)), lerp(0.34, 0.74, v.powf(1.2)), lerp(0.80, 0.56, v)];
        let (sx, sy) = (0.30 + 0.45 * seed, 0.46);
        let d = (((u - sx) * aspect).powi(2) + (v - sy).powi(2)).sqrt();
        let halo = (1.0 - d / 0.35).clamp(0.0, 1.0).powi(2);
        let disc = (1.0 - (d - 0.07) * 60.0).clamp(0.0, 1.0);
        for k in 0..3 {
            c[k] = lerp(c[k], [1.0, 0.93, 0.78][k], halo * 0.45);
            c[k] = lerp(c[k], [1.0, 0.97, 0.88][k], disc);
        }
        let far = 0.64 + 0.05 * (u * 6.0 + seed * 5.0).sin() + 0.02 * (u * 19.0).sin();
        let near = 0.80 + 0.06 * (u * 4.0 + 1.3 + seed * 3.0).sin() + 0.02 * (u * 23.0 + 2.0).sin();
        if v > far {
            let t = ((v - far) * 3.0).clamp(0.0, 1.0);
            c = [lerp(0.38, 0.30, t), lerp(0.50, 0.44, t), lerp(0.62, 0.52, t)];
        }
        if v > near {
            let t = ((v - near) * 4.0).clamp(0.0, 1.0);
            c = [lerp(0.16, 0.08, t), lerp(0.34, 0.22, t), lerp(0.22, 0.14, t)];
        }
        [(c[0] * 255.0 + 0.5) as u8, (c[1] * 255.0 + 0.5) as u8, (c[2] * 255.0 + 0.5) as u8, 255]
    })
}

fn heart() -> ImageData {
    const ROWS: [&str; 8] = ["01100110", "11111111", "11111111", "11111111", "01111110", "00111100", "00011000", "00000000"];
    ImageData::from_fn(8, 8, |x, y| if ROWS[y as usize].as_bytes()[x as usize] == b'1' { [236, 72, 120, 255] } else { [0, 0, 0, 0] })
}

/// A 24-point icon rasterised as an alpha mask with 4x4 supersampling.
fn icon(inside: impl Fn(f32, f32) -> bool) -> ImageData {
    ImageData::from_fn(24, 24, |x, y| {
        let mut hits = 0;
        for sy in 0..4 {
            for sx in 0..4 {
                if inside(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) {
                    hits += 1;
                }
            }
        }
        [0, 0, 0, (hits * 255 / 16) as u8]
    })
}

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (ab, ap) = ((b.0 - a.0, b.1 - a.1), (p.0 - a.0, p.1 - a.1));
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
    ((p.0 - (a.0 + ab.0 * t)).powi(2) + (p.1 - (a.1 + ab.1 * t)).powi(2)).sqrt()
}

/// A soft round sticker with a translucent rim, to show compositing over a pattern.
fn sticker() -> ImageData {
    ImageData::from_fn(48, 48, |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - 24.0, y as f32 + 0.5 - 24.0);
        let d = (dx * dx + dy * dy).sqrt() / 24.0;
        let a = (1.0 - d).clamp(0.0, 1.0);
        let alpha = (a.sqrt() * 255.0) as u8;
        [37, 99, 235, alpha]
    })
}

struct Sheet {
    photos: [vui_core::ImageId; 2],
    hash: vui_core::ImageId,
    heart: vui_core::ImageId,
    icons: [vui_core::ImageId; 4],
    sticker: vui_core::ImageId,
}

fn panel(s: &mut Scene, res: &Resources, theme: &Theme, ox: f32, sheet: &Sheet) {
    let p = &theme.palette;
    s.quad(Quad::new(rect(ox, 0.0, 280.0, 372.0)).fill(p.bg));
    let label = |s: &mut Scene, text: &str, x: f32, y: f32, color: Color| {
        let l = res.fonts.layout_cached(text, &TextStyle::from_token(theme.types.small).weight(500), None);
        s.text(l, pt(ox + x, y), color, None);
    };

    // avatars: cover-fit crops cut to circles, one with an accent ring for "selected"
    label(s, "Avatars", 16.0, 14.0, p.text_3);
    for i in 0..4 {
        let at = rect(ox + 16.0 + i as f32 * 60.0, 36.0, 48.0, 48.0);
        let id = [sheet.photos[0], sheet.photos[1], sheet.hash, sheet.photos[0]][i];
        let (dest, uv) = fit_image(res.images.size(id).unwrap(), at, Fit::Cover);
        let ring = if i == 3 { (2.0, p.accent) } else { (1.0, p.ring) };
        s.image(ImageCmd::new(id, dest).uv(uv).radii(PILL).ring(ring.0, ring.1));
    }

    // tiles: continuous corners, a hairline ring, and a label over a scrim
    label(s, "Tiles", 16.0, 98.0, p.text_3);
    let widths = [104.0, 72.0, 56.0];
    let sources = [sheet.photos[0], sheet.photos[0], sheet.photos[1]];
    let mut x = ox + 16.0;
    for (i, w) in widths.iter().enumerate() {
        let at = rect(x, 120.0, *w, 72.0);
        let (dest, uv) = fit_image(res.images.size(sources[i]).unwrap(), at, Fit::Cover);
        s.image(ImageCmd::new(sources[i], dest).uv(uv).radii(theme.radii.card).smooth(theme.radii.smooth).ring(1.0, p.ring));
        x += w + 8.0;
    }
    s.quad(Quad::new(rect(ox + 16.0, 168.0, 104.0, 24.0)).fill(p.scrim).radii(Corners::new(0.0, 0.0, 12.0, 12.0)).smooth(theme.radii.smooth));
    let l = res.fonts.layout_cached("Screenshot", &TextStyle::from_token(theme.types.small).weight(500), None);
    s.text(l, pt(ox + 24.0, 172.0), Color::WHITE, None);

    // zoom: the same 8x8 sprite at 7.5x, with three sampling modes
    label(s, "Zoom", 16.0, 208.0, p.text_3);
    for (i, (name, sampling)) in [("smooth", Sampling::Smooth), ("sharp", Sampling::Sharp), ("nearest", Sampling::Nearest)].into_iter().enumerate() {
        let x = ox + 16.0 + i as f32 * 76.0;
        s.quad(Quad::new(rect(x, 228.0, 60.0, 60.0)).fill(p.wash).radii(8.0).smooth(theme.radii.smooth));
        s.image(ImageCmd::new(sheet.heart, rect(x, 228.0, 60.0, 60.0)).sampling(sampling));
        label(s, name, x, 294.0, p.text_3);
    }

    // icons tinted from the palette, and a translucent sticker over a pattern
    label(s, "Icons", 16.0, 320.0, p.text_3);
    for (i, tint) in [p.text, p.text_2, p.accent, p.danger].into_iter().enumerate() {
        s.image(ImageCmd::new(sheet.icons[i], rect(ox + 16.0 + i as f32 * 32.0, 340.0, 24.0, 24.0)).mask(tint));
    }
    for k in 0..11 {
        let fill = if k % 2 == 0 { p.wash } else { p.hover };
        s.quad(Quad::new(rect(ox + 160.0 + k as f32 * 8.0, 332.0, 8.0, 32.0)).fill(fill));
    }
    s.image(ImageCmd::new(sheet.sticker, rect(ox + 180.0, 328.0, 40.0, 40.0)));
}

/// Pictures in both themes: avatars, tiles, zoom modes, tinted icons and a translucent sticker.
#[test]
fn images_sheet() {
    let res = Resources::bundled_only();
    let wide = photo(240, 160, 0.0);
    let tall = photo(120, 180, 0.7);
    let hash = ImageData::from_thumb_hash(&photo(160, 120, 0.4).thumb_hash()).unwrap();
    let sheet = Sheet {
        photos: [res.images.insert(wide), res.images.insert(tall)],
        hash: res.images.insert(hash),
        heart: res.images.insert(heart()),
        icons: [
            res.images.insert(icon(|x, y| ((x - 12.0).hypot(y - 12.0) - 8.0).abs() <= 1.25)),
            res.images.insert(icon(|x, y| (x - 12.0).abs() <= 1.25 && (y - 12.0).abs() <= 8.0 || (y - 12.0).abs() <= 1.25 && (x - 12.0).abs() <= 8.0)),
            res.images.insert(icon(|x, y| segment_distance((x, y), (9.0, 5.0), (16.0, 12.0)) <= 1.25 || segment_distance((x, y), (16.0, 12.0), (9.0, 19.0)) <= 1.25)),
            res.images.insert(icon(|x, y| (x - 12.0).hypot(y - 12.0) <= 6.0)),
        ],
        sticker: res.images.insert(sticker()),
    };
    let mut s = Scene::new(size(560.0, 372.0), 2.0).with_clear(Color::WHITE);
    panel(&mut s, &res, &Theme::light(), 0.0, &sheet);
    panel(&mut s, &res, &Theme::dark(), 280.0, &sheet);
    let img = render_with(&s, &res);
    assert_golden!("images_sheet", img);
}
