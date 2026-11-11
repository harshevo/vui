//! Drawing pictures: corners, rings, crops, masks, opacity, transparency, placeholders, layering
//! and clipping.

use super::*;

#[test]
fn rounded_corners_cut_the_picture_and_continuous_corners_cut_less() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(8, 8, |_, _| [37, 99, 235, 255]));
    let draw = |smooth: f32| {
        let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
        s.image(ImageCmd::new(id, rect(0.0, 0.0, 40.0, 40.0)).radii(12.0).smooth(smooth));
        render_with(&s, &res)
    };
    let round = draw(0.0);
    assert_eq!(round.pixel(0, 0), WHITE, "the corner is cut away");
    assert!(near(round.pixel(20, 20), [37, 99, 235, 255], 1));
    assert!(near(round.pixel(20, 0), [37, 99, 235, 255], 1), "the straight edge is untouched");
    assert!(round.pixel(2, 2)[1] > 250, "a circular corner of radius 12 has not reached (2,2): {:?}", round.pixel(2, 2));
    let squircle = draw(0.6);
    assert!(squircle.pixel(2, 2)[1] < 120, "a continuous corner is fuller there: {:?}", squircle.pixel(2, 2));
}

#[test]
fn a_ring_is_drawn_over_the_edge_inside_the_rect() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(4, 4, |_, _| [255, 0, 0, 255]));
    let mut s = Scene::new(size(60.0, 24.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(2.0, 2.0, 20.0, 20.0)).ring(2.0, Color::BLACK));
    s.image(ImageCmd::new(id, rect(34.0, 2.0, 20.0, 20.0)).ring(2.0, Color::rgba(1.0, 1.0, 1.0, 0.5)));
    let img = render_with(&s, &res);
    assert_eq!(img.pixel(12, 2), [0, 0, 0, 255], "ring on the top edge");
    assert_eq!(img.pixel(12, 3), [0, 0, 0, 255]);
    assert_eq!(img.pixel(12, 4), [255, 0, 0, 255], "the picture shows inside the ring");
    assert_eq!(img.pixel(12, 1), WHITE, "nothing outside the rect");
    assert!(near(img.pixel(44, 2), [255, 128, 128, 255], 2), "a translucent ring tints the picture: {:?}", img.pixel(44, 2));
}

#[test]
fn a_crop_shows_only_its_part_without_bleeding_from_the_neighbours() {
    let res = Resources::bundled_only();
    let quad = ImageData::from_fn(2, 2, |x, y| match (x, y) {
        (0, 0) => [255, 0, 0, 255],
        (1, 0) => [0, 255, 0, 255],
        (0, 1) => [0, 0, 255, 255],
        _ => [255, 255, 0, 255],
    });
    let id = res.images.insert(quad);
    for sampling in [Sampling::Smooth, Sampling::Sharp] {
        let mut s = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::WHITE);
        s.image(ImageCmd::new(id, rect(2.0, 2.0, 16.0, 16.0)).uv(rect(0.5, 0.0, 0.5, 0.5)).sampling(sampling));
        let img = render_with(&s, &res);
        for y in 2..18 {
            for x in 2..18 {
                assert!(near(img.pixel(x, y), [0, 255, 0, 255], 1), "{sampling:?} ({x},{y}) is {:?}", img.pixel(x, y));
            }
        }
    }
}

#[test]
fn a_flipped_crop_mirrors_the_picture() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(2, 1, |x, _| if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let mut s = Scene::new(size(16.0, 8.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 16.0, 8.0)).uv(rect(1.0, 0.0, -1.0, 1.0)).sampling(Sampling::Nearest));
    let img = render_with(&s, &res);
    assert!(near(img.pixel(2, 4), [0, 0, 255, 255], 1), "blue is now on the left: {:?}", img.pixel(2, 4));
    assert!(near(img.pixel(13, 4), [255, 0, 0, 255], 1));
}

#[test]
fn a_mask_takes_the_tint_and_ignores_the_picture_colour() {
    let res = Resources::bundled_only();
    let icon = ImageData::from_fn(8, 8, |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - 4.0, y as f32 + 0.5 - 4.0);
        [255, 0, 0, if dx * dx + dy * dy <= 9.0 { 255 } else { 0 }]
    });
    let id = res.images.insert(icon);
    let mut s = Scene::new(size(16.0, 16.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(4.0, 4.0, 8.0, 8.0)).mask(ACCENT).sampling(Sampling::Nearest));
    let img = render_with(&s, &res);
    assert!(near(img.pixel(8, 8), [37, 99, 235, 255], 1), "tinted, not red: {:?}", img.pixel(8, 8));
    assert_eq!(img.pixel(4, 4), WHITE, "outside the glyph is transparent");
}

#[test]
fn opacity_fades_the_picture_into_what_is_behind() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(2, 2, |_, _| [0, 0, 0, 255]));
    let mut s = Scene::new(size(8.0, 8.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 8.0, 8.0)).opacity(0.5));
    let img = render_with(&s, &res);
    let v = img.pixel(4, 4)[0];
    assert!((127..=128).contains(&v), "half black over white is mid grey, got {v}");
}

#[test]
fn partly_transparent_pixels_blend_with_what_is_behind_them() {
    let res = Resources::bundled_only();
    let id = res.images.insert(solid(200, 100, 50, 128));
    let a = 128.0 / 255.0;
    let mix = |c: f32, bg: f32| (c * a + bg * (1.0 - a)).round() as u8;

    let mut over_white = Scene::new(size(4.0, 4.0), 1.0).with_clear(Color::WHITE);
    over_white.image(ImageCmd::new(id, rect(1.0, 1.0, 2.0, 2.0)));
    let want = [mix(200.0, 255.0), mix(100.0, 255.0), mix(50.0, 255.0), 255];
    let got = render_with(&over_white, &res).pixel(1, 1);
    assert!(near(got, want, 2), "over white: got {got:?}, want {want:?}");

    let mut over_black = Scene::new(size(4.0, 4.0), 1.0).with_clear(Color::BLACK);
    over_black.image(ImageCmd::new(id, rect(1.0, 1.0, 2.0, 2.0)));
    let want = [mix(200.0, 0.0), mix(100.0, 0.0), mix(50.0, 0.0), 255];
    let got = render_with(&over_black, &res).pixel(2, 2);
    assert!(near(got, want, 2), "over black: got {got:?}, want {want:?}");
}

#[test]
fn transparent_texels_neither_darken_nor_tint_their_neighbours() {
    let res = Resources::bundled_only();
    // opaque white beside a fully transparent texel whose hidden colour is pure red
    let id = res.images.insert(ImageData::from_fn(2, 1, |x, _| if x == 0 { [255, 255, 255, 255] } else { [255, 0, 0, 0] }));
    let mut s = Scene::new(size(32.0, 4.0), 1.0).with_clear(Color::BLACK);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 32.0, 4.0)));
    let img = render_with(&s, &res);
    // over black, the picture is white at the alpha the filter produces; the red must never show
    for x in 8..24 {
        let p = img.pixel(x, 2);
        assert!(p[0].abs_diff(p[1]) <= 2 && p[1].abs_diff(p[2]) <= 2, "x={x}: hidden red leaked into {p:?}");
    }
    // halfway between the texel centres the alpha is 50%: 128 in the blend space, not the brighter 188
    let mid = img.pixel(15, 2)[0];
    assert!((120..=136).contains(&mid), "the midpoint of a white-to-clear ramp over black is mid grey, got {mid}");
    // and the ramp falls steadily
    let ramp: Vec<u8> = (8..24).map(|x| img.pixel(x, 2)[0]).collect();
    assert!(ramp.windows(2).all(|w| w[0] >= w[1]), "{ramp:?}");
}

#[test]
fn a_thumb_hash_placeholder_is_a_soft_version_of_the_picture() {
    let res = Resources::bundled_only();
    let src = photo(160, 120, 0.4);
    let blur = ImageData::from_thumb_hash(&src.thumb_hash()).unwrap();
    let id = res.images.insert(blur);
    let mut s = Scene::new(size(96.0, 72.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 96.0, 72.0)));
    let img = render_with(&s, &res);

    let mean = |im: &Image| {
        let mut sum = [0u64; 3];
        for y in 0..im.height {
            for x in 0..im.width {
                let p = im.pixel(x, y);
                for c in 0..3 {
                    sum[c] += p[c] as u64;
                }
            }
        }
        let n = (im.width * im.height) as u64;
        [sum[0] / n, sum[1] / n, sum[2] / n]
    };
    let original = {
        let mut sum = [0u64; 3];
        for p in src.rgba.as_chunks::<4>().0 {
            for c in 0..3 {
                sum[c] += p[c] as u64;
            }
        }
        let n = (src.width * src.height) as u64;
        [sum[0] / n, sum[1] / n, sum[2] / n]
    };
    let blurred = mean(&img);
    for c in 0..3 {
        assert!(blurred[c].abs_diff(original[c]) < 24, "channel {c}: {blurred:?} vs {original:?}");
    }
    let mut steepest = 0;
    for y in 0..img.height {
        for x in 1..img.width {
            let (a, b) = (img.pixel(x - 1, y), img.pixel(x, y));
            steepest = steepest.max(a[0].abs_diff(b[0])).max(a[1].abs_diff(b[1])).max(a[2].abs_diff(b[2]));
        }
    }
    assert!(steepest <= 12, "a placeholder has no hard edges, steepest step {steepest}");
}

#[test]
fn images_layer_with_shapes_in_painter_order() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(2, 2, |_, _| [255, 0, 0, 255]));
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 40.0, 40.0)).fill(Color::BLACK));
    s.image(ImageCmd::new(id, rect(10.0, 10.0, 20.0, 20.0)));
    s.quad(Quad::new(rect(0.0, 0.0, 20.0, 40.0)).fill(Color::rgba(1.0, 1.0, 1.0, 0.5)));
    let img = render_with(&s, &res);
    assert!(near(img.pixel(5, 5), [128, 128, 128, 255], 2), "veil over black: {:?}", img.pixel(5, 5));
    assert!(near(img.pixel(15, 15), [255, 128, 128, 255], 2), "veil over the picture: {:?}", img.pixel(15, 15));
    assert_eq!(img.pixel(25, 25), [255, 0, 0, 255], "the picture over black");
    assert_eq!(img.pixel(35, 35), [0, 0, 0, 255]);
}

#[test]
fn a_picture_is_cut_by_its_clip() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(2, 2, |_, _| [255, 0, 0, 255]));
    let mut s = Scene::new(size(40.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 40.0, 20.0)).clip(Some(rect(0.0, 0.0, 20.0, 20.0))));
    let img = render_with(&s, &res);
    assert_eq!(img.pixel(19, 10), [255, 0, 0, 255]);
    assert_eq!(img.pixel(20, 10), WHITE);
}
