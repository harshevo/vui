//! Sampling: pixels reproduced, shrinking in linear light, and sharp versus smooth zoom.

use super::*;

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
