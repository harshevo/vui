//! Rendering tests for pictures: colour accuracy, filtering in linear light, crops and corner shapes,
//! transparency, the GPU texture cache, layering with shapes, and a golden sheet to read by eye.

use vui_core::theme::PILL;
use vui_core::{
    Color, Corners, Fit, ImageCmd, ImageData, Quad, Resources, Sampling, Scene, TextStyle, Theme, fit_image, pt, rect, size,
};
use vui_render::testing::{render_with, renderer_for};
use vui_render::{Image, assert_golden};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const ACCENT: Color = Color::rgb(37.0 / 255.0, 99.0 / 255.0, 235.0 / 255.0);

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= tol)
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> ImageData {
    ImageData::from_fn(1, 1, |_, _| [r, g, b, a])
}

#[test]
fn an_unscaled_image_reproduces_its_pixels() {
    let res = Resources::bundled_only();
    let src = ImageData::from_fn(4, 4, |x, y| [(x * 70 + 5) as u8, (y * 60 + 20) as u8, ((x + y) * 30 + 40) as u8, 255]);
    let id = res.images.insert(src.clone());
    for sampling in [Sampling::Smooth, Sampling::Sharp, Sampling::Nearest] {
        let mut s = Scene::new(size(8.0, 8.0), 1.0).with_clear(Color::WHITE);
        s.image(ImageCmd::new(id, rect(2.0, 2.0, 4.0, 4.0)).sampling(sampling));
        let img = render_with(&s, &res);
        for y in 0..4 {
            for x in 0..4 {
                let (got, want) = (img.pixel(2 + x, 2 + y), src.pixel(x, y));
                assert!(near(got, want, 1), "{sampling:?} texel ({x},{y}): got {got:?}, want {want:?}");
            }
        }
        assert_eq!(img.pixel(1, 1), WHITE, "outside the picture is untouched");
        assert_eq!(img.pixel(6, 6), WHITE);
    }
}

#[test]
fn shrinking_a_checkerboard_averages_light_not_code_values() {
    let res = Resources::bundled_only();
    let checker = ImageData::from_fn(64, 64, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
    let id = res.images.insert(checker);
    let mut s = Scene::new(size(16.0, 16.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(4.0, 4.0, 8.0, 8.0)));
    let img = render_with(&s, &res);
    for y in 5..11 {
        for x in 5..11 {
            let v = img.pixel(x, y)[0];
            // half the light is code value 188; averaging the code values would give a muddy 128
            assert!((184..=192).contains(&v), "pixel ({x},{y}) is {v}");
        }
    }
}

#[test]
fn sharp_zoom_keeps_texel_edges_crisp_where_smooth_zoom_blurs_them() {
    let res = Resources::bundled_only();
    let two = ImageData::from_fn(2, 1, |x, _| if x == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
    let id = res.images.insert(two);
    // pixels strictly between the two texel colours on the middle row
    let in_between = |sampling| {
        let mut s = Scene::new(size(64.0, 8.0), 1.0).with_clear(Color::WHITE);
        s.image(ImageCmd::new(id, rect(0.0, 0.0, 64.0, 8.0)).sampling(sampling));
        let img = render_with(&s, &res);
        assert_eq!(img.pixel(2, 4)[0], 0, "left texel is black");
        assert_eq!(img.pixel(61, 4)[0], 255, "right texel is white");
        (0..64).filter(|&x| (12..=243).contains(&img.pixel(x, 4)[0])).count()
    };
    let (smooth, sharp, nearest) = (in_between(Sampling::Smooth), in_between(Sampling::Sharp), in_between(Sampling::Nearest));
    assert!(smooth >= 20, "smooth zoom ramps across the texels, {smooth} pixels in between");
    assert!(sharp <= 2, "sharp zoom blends over about one pixel, {sharp} pixels in between");
    assert_eq!(nearest, 0, "nearest never blends");
}

#[test]
fn sharp_zoom_is_crisp_at_device_scale_two() {
    let res = Resources::bundled_only();
    let src = ImageData::from_fn(4, 4, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
    let id = res.images.insert(src);
    let mut s = Scene::new(size(12.0, 12.0), 2.0).with_clear(Color::rgb(0.5, 0.5, 0.5));
    // 8 logical points = 16 device pixels = 4 device pixels per texel
    s.image(ImageCmd::new(id, rect(2.0, 2.0, 8.0, 8.0)).sampling(Sampling::Sharp));
    let img = render_with(&s, &res);
    assert_eq!((img.width, img.height), (24, 24));
    assert_eq!(img.pixel(4, 4)[0], 0, "first texel starts at device pixel 4");
    assert_eq!(img.pixel(7, 7)[0], 0, "and ends at 7");
    assert_eq!(img.pixel(8, 4)[0], 255, "the next texel is white");
    assert_eq!(img.pixel(8, 8)[0], 0);
    assert_eq!(img.pixel(3, 3)[0], 128, "outside is the clear colour");
}

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

#[test]
fn textures_are_uploaded_lazily_and_evicted_least_recently_used_first() {
    let res = Resources::bundled_only();
    let ids: Vec<_> = (0..3u8).map(|i| res.images.insert(ImageData::from_fn(64, 64, |_, _| [i * 80, 40, 200, 255]))).collect();
    let mut renderer = renderer_for(&res);
    let one_texture = 64 * 64 * 4 * 4 / 3;
    renderer.set_image_budget(one_texture * 2 + one_texture / 2);
    assert_eq!(renderer.image_stats().resident, 0, "nothing is uploaded until it is drawn");

    let draw = |renderer: &mut vui_render::Renderer, which: &[usize]| {
        let mut s = Scene::new(size(64.0, 64.0), 1.0).with_clear(Color::WHITE);
        for &i in which {
            s.image(ImageCmd::new(ids[i], rect(0.0, 0.0, 64.0, 64.0)));
        }
        renderer.render_to_image(&s).unwrap();
        renderer.image_stats()
    };

    let st = draw(&mut renderer, &[0]);
    assert_eq!((st.resident, st.uploads, st.evictions), (1, 1, 0));
    let st = draw(&mut renderer, &[0]);
    assert_eq!(st.uploads, 1, "a resident texture is reused");
    let st = draw(&mut renderer, &[1]);
    assert_eq!((st.resident, st.uploads, st.evictions), (2, 2, 0));
    let st = draw(&mut renderer, &[2]);
    assert_eq!((st.resident, st.uploads, st.evictions), (2, 3, 1), "over budget: the oldest (0) goes");
    let st = draw(&mut renderer, &[0]);
    assert_eq!((st.resident, st.uploads, st.evictions), (2, 4, 2), "0 comes back, 1 is now the oldest");
    let st = draw(&mut renderer, &[2]);
    assert_eq!(st.uploads, 4, "2 stayed resident");
    assert!(st.resident_bytes <= one_texture * 2 + one_texture / 2);
}

#[test]
fn pictures_drawn_this_frame_are_never_evicted_mid_frame() {
    let res = Resources::bundled_only();
    let ids: Vec<_> = (0..3u8).map(|i| res.images.insert(ImageData::from_fn(32, 32, |_, _| [i * 90, 10, 10, 255]))).collect();
    let mut renderer = renderer_for(&res);
    renderer.set_image_budget(1);
    let mut s = Scene::new(size(32.0, 96.0), 1.0).with_clear(Color::WHITE);
    for (i, id) in ids.iter().enumerate() {
        s.image(ImageCmd::new(*id, rect(0.0, i as f32 * 32.0, 32.0, 32.0)));
    }
    let img = renderer.render_to_image(&s).unwrap();
    assert_eq!(img.pixel(16, 16)[0], 0);
    assert!(img.pixel(16, 48)[0] > 80 && img.pixel(16, 80)[0] > 160, "all three drew despite the tiny budget");
    assert_eq!(renderer.image_stats().resident, 3);
    renderer.render_to_image(&Scene::new(size(8.0, 8.0), 1.0)).unwrap();
    let st = renderer.image_stats();
    assert_eq!((st.resident, st.evictions), (0, 3), "an idle frame lets the cache shrink to its budget");
}

#[test]
fn an_unregistered_picture_draws_nothing_and_frees_its_texture() {
    let res = Resources::bundled_only();
    let id = res.images.insert(ImageData::from_fn(8, 8, |_, _| [255, 0, 0, 255]));
    let mut renderer = renderer_for(&res);
    let mut s = Scene::new(size(8.0, 8.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 8.0, 8.0)));
    assert_eq!(renderer.render_to_image(&s).unwrap().pixel(4, 4), [255, 0, 0, 255]);
    assert_eq!(renderer.image_stats().resident, 1);
    res.images.remove(id);
    assert_eq!(renderer.render_to_image(&s).unwrap().pixel(4, 4), WHITE, "the removed picture is gone");
    assert_eq!(renderer.image_stats().resident, 0);
}

#[test]
fn a_picture_larger_than_the_gpu_allows_is_shrunk_to_fit() {
    let res = Resources::bundled_only();
    let limit = vui_render::testing::shared_gpu().device.limits().max_texture_dimension_2d;
    let wide = limit + 64;
    let id = res.images.insert(ImageData::from_fn(wide, 2, |x, _| if x < wide / 2 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let mut s = Scene::new(size(32.0, 4.0), 1.0).with_clear(Color::WHITE);
    s.image(ImageCmd::new(id, rect(0.0, 0.0, 32.0, 4.0)));
    let img = render_with(&s, &res);
    assert!(near(img.pixel(4, 2), [255, 0, 0, 255], 2), "{:?}", img.pixel(4, 2));
    assert!(near(img.pixel(28, 2), [0, 0, 255, 255], 2), "{:?}", img.pixel(28, 2));
}

// ---- the sheet ----------------------------------------------------------------------------------

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A landscape to look at: sky, sun, two ridges. `seed` moves the sun and the hills.
fn photo(w: u32, h: u32, seed: f32) -> ImageData {
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
