//! Textures on the GPU: lazy uploads, eviction under a budget, and pictures too large for it.

use super::*;

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
