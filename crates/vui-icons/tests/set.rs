//! What must hold for every icon in the set, and for the mechanics around them.

use std::collections::HashSet;

use vui_core::Images;
use vui_icons::{Icon, SPINNER_FRAMES, Weight, by_name, icon, set, spinner};

fn flipped_x(m: &vui_icons::Mask) -> Vec<u8> {
    let w = m.width as usize;
    m.alpha.chunks(w).flat_map(|row| row.iter().rev().copied()).collect()
}

fn flipped_y(m: &vui_icons::Mask) -> Vec<u8> {
    let w = m.width as usize;
    m.alpha.chunks(w).rev().flatten().copied().collect()
}

fn close(a: &[u8], b: &[u8], tolerance: u8) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= tolerance)
}

#[test]
fn every_icon_parses() {
    for icon in set::ALL {
        icon.validate().unwrap_or_else(|e| panic!("{}: {e}", icon.name()));
    }
}

#[test]
fn names_are_unique_kebab_case_and_found_again() {
    let mut seen = HashSet::new();
    for icon in set::ALL {
        let name = icon.name();
        assert!(!name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "{name:?}");
        assert!(seen.insert(name), "{name} is declared twice");
        assert_eq!(by_name(name), Some(*icon));
    }
    assert!(set::ALL.len() >= 60, "{} icons", set::ALL.len());
    assert_eq!(by_name("no-such-icon"), None);
}

#[test]
fn every_icon_draws_something_but_not_everything() {
    for icon in set::ALL {
        for px in [16, 24, 32, 48] {
            let area = (px * px) as f32;
            let ink = icon.render(px, Weight::Regular).ink();
            assert!(ink > area * 0.012, "{} at {px}px has almost nothing in it: {ink}", icon.name());
            assert!(ink < area * 0.35, "{} at {px}px is a blob: {ink}", icon.name());
        }
    }
}

#[test]
fn every_icon_keeps_clear_of_the_edge() {
    for icon in set::ALL {
        for px in [16, 24, 32, 40, 48] {
            for weight in [Weight::Light, Weight::Bold] {
                let m = icon.render(px, weight);
                let last = px - 1;
                for i in 0..px {
                    for (x, y) in [(i, 0), (i, last), (0, i), (last, i)] {
                        assert_eq!(m.get(x, y), 0, "{} at {px}px {weight:?} touches the edge at ({x}, {y})", icon.name());
                    }
                }
            }
        }
    }
}

#[test]
fn icons_scale_without_changing_weight() {
    // Four times the pixels, so four times the ink at every size, to within the edge pixels.
    for icon in set::ALL {
        let small = icon.render(24, Weight::Regular).ink();
        let large = icon.render(96, Weight::Regular).ink() / 16.0;
        assert!((small - large).abs() / large < 0.06, "{}: {small} at 24px vs {large} at 96px scaled back", icon.name());
    }
}

#[test]
fn heavier_weights_have_more_ink() {
    for icon in [set::SEARCH, set::PLUS, set::SETTINGS, set::FOLDER, set::CHECK] {
        let ink: Vec<f32> = [Weight::Light, Weight::Regular, Weight::Medium, Weight::Bold].map(|w| icon.render(32, w).ink()).into();
        assert!(ink.windows(2).all(|w| w[0] < w[1]), "{}: {ink:?}", icon.name());
    }
}

#[test]
fn rendering_is_repeatable() {
    for icon in set::ALL {
        assert_eq!(icon.render(24, Weight::Regular), icon.render(24, Weight::Regular), "{}", icon.name());
    }
}

#[test]
fn symmetric_icons_are_symmetric() {
    // Left-right: the plus, minus, x and things that point up or down.
    for icon in [set::PLUS, set::MINUS, set::X, set::CHEVRON_UP, set::CHEVRON_DOWN, set::ARROW_UP, set::ARROW_DOWN, set::SUN, set::MORE, set::HOME] {
        for px in [16, 24, 32] {
            let m = icon.render(px, Weight::Regular);
            assert!(close(&m.alpha, &flipped_x(&m), 1), "{} at {px}px is not the same mirrored left to right", icon.name());
        }
    }
    // Top-bottom: the plus, x and things that point left or right.
    for icon in [set::PLUS, set::X, set::MORE, set::CHEVRON_LEFT, set::CHEVRON_RIGHT, set::ARROW_LEFT, set::ARROW_RIGHT, set::SUN, set::MINUS] {
        for px in [16, 24, 32] {
            let m = icon.render(px, Weight::Regular);
            assert!(close(&m.alpha, &flipped_y(&m), 1), "{} at {px}px is not the same mirrored top to bottom", icon.name());
        }
    }
}

#[test]
fn lines_sit_on_whole_pixels_where_the_grid_allows() {
    // At 24 px a bold (2 unit) stroke centred on a grid line is exactly two pixels wide.
    let plus = set::PLUS.render(24, Weight::Bold);
    for y in [8, 15] {
        assert_eq!((plus.get(10, y), plus.get(11, y), plus.get(12, y), plus.get(13, y)), (0, 255, 255, 0), "row {y}");
    }
    for x in [8, 15] {
        assert_eq!((plus.get(x, 10), plus.get(x, 11), plus.get(x, 12), plus.get(x, 13)), (0, 255, 255, 0), "column {x}");
    }
}

#[test]
fn solid_shapes_are_filled_and_line_shapes_are_hollow() {
    assert_eq!(set::PLAY.render(32, Weight::Regular).get(14, 16), 255);
    assert_eq!(set::STOP.render(32, Weight::Regular).get(16, 16), 255);
    assert_eq!(set::CLOCK.render(32, Weight::Regular).get(10, 10), 0, "a ring is empty inside");
    assert_eq!(set::INFO.render(48, Weight::Regular).get(14, 24), 0);
}

#[test]
fn icons_made_of_your_own_path_data_work_and_bad_data_is_reported() {
    const SQUARE: Icon = icon!("square", Stroke "M6 6h12v12H6z");
    const SOLID: Icon = icon!("solid", Fill "M6 6h12v12H6z");
    const BOTH: Icon = icon!("both", Both "M6 6h12v12H6z");
    const BAD: Icon = icon!("bad", Stroke "M6 6h12v12H6z", Stroke "M1 1 Q");
    assert!(SQUARE.validate().is_ok());
    assert_eq!(SQUARE.render(24, Weight::Regular).get(12, 12), 0);
    assert_eq!(SOLID.render(24, Weight::Regular).get(12, 12), 255);
    assert!(BOTH.render(24, Weight::Regular).ink() > SOLID.render(24, Weight::Regular).ink());
    let err = BAD.validate().unwrap_err();
    assert_eq!(err.message, "expected a number");
    // The good layer still draws; the bad one is skipped.
    assert_eq!(BAD.render(24, Weight::Regular), SQUARE.render(24, Weight::Regular));
}

#[test]
fn icons_compare_by_name() {
    const A: Icon = icon!("same", Stroke "M0 0h1");
    const B: Icon = icon!("same", Stroke "M0 0h2");
    assert_eq!(A, B);
    assert_ne!(A, set::CHECK);
    assert_eq!(format!("{:?}", set::CHECK), "Icon(check)");
}

#[test]
fn an_icon_becomes_a_mask_image_made_once_per_size_and_weight() {
    let images = Images::new();
    let a = set::CHECK.image(&images, 32, Weight::Regular);
    assert_eq!(set::CHECK.image(&images, 32, Weight::Regular), a, "the same request, the same image");
    assert_eq!(images.len(), 1);
    let sizes = [set::CHECK.image(&images, 48, Weight::Regular), set::CHECK.image(&images, 32, Weight::Bold), set::PLUS.image(&images, 32, Weight::Regular)];
    assert!(sizes.iter().all(|&id| id != a), "a different size, weight or icon is a different image");
    assert_eq!(images.len(), 4);
    assert_eq!(images.size(a).map(|s| (s.w, s.h)), Some((32.0, 32.0)));
    // It is a mask: white with the coverage as alpha.
    let data = images.get(a).unwrap();
    assert!(data.rgba.chunks(4).all(|p| p[..3] == [255, 255, 255]));
    assert!(data.rgba.chunks(4).any(|p| p[3] == 255));
    // Dropping it from the store remakes it on the next request.
    images.remove(a);
    let again = set::CHECK.image(&images, 32, Weight::Regular);
    assert!(images.get(again).is_some());
}

// ---- the spinner ---------------------------------------------------------------------------------

/// The angle, in degrees clockwise from straight up, of the middle of the bright part.
fn arc_angle(frame: u32) -> f32 {
    let m = spinner::render(64, frame, Weight::Regular);
    let (mut sx, mut sy, mut total) = (0.0f32, 0.0f32, 0.0f32);
    for y in 0..64 {
        for x in 0..64 {
            let a = m.get(x, y) as f32;
            if a > 200.0 {
                let (dx, dy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 32.0);
                // unit vectors average to the middle of an arc even when it wraps past 180 degrees
                let len = dx.hypot(dy);
                sx += dx / len * a;
                sy += dy / len * a;
                total += a;
            }
        }
    }
    assert!(total > 0.0);
    (sy.atan2(sx).to_degrees() + 90.0).rem_euclid(360.0)
}

#[test]
fn the_spinner_arc_travels_clockwise_one_turn_in_sixty_frames() {
    assert_eq!(SPINNER_FRAMES, 60);
    let start = arc_angle(0);
    for frame in [0u32, 5, 15, 30, 45, 59] {
        let want = (start + 6.0 * frame as f32).rem_euclid(360.0);
        let got = arc_angle(frame);
        let off = ((got - want + 540.0).rem_euclid(360.0) - 180.0).abs();
        assert!(off < 3.0, "frame {frame}: the arc is at {got:.1} degrees, expected {want:.1}");
    }
    assert!((start - 50.0).abs() < 3.0, "frame 0 starts at the top and covers 100 degrees: {start}");
}

#[test]
fn the_spinner_wraps_and_keeps_a_faint_ring() {
    assert_eq!(spinner::render(32, 0, Weight::Regular), spinner::render(32, SPINNER_FRAMES, Weight::Regular));
    assert_ne!(spinner::render(32, 0, Weight::Regular), spinner::render(32, 1, Weight::Regular));
    // Frame 0's arc is on the upper right; the lower left carries only the ring.
    let m = spinner::render(64, 0, Weight::Regular);
    let ring = m.get(32 - 17, 32 + 17).max(m.get(32 - 17, 32 + 16));
    assert!((40..=60).contains(&ring), "the ring is about a fifth strong: {ring}");
    assert!(m.get(32 + 22, 32 - 8) > 200, "the arc is solid");
}

#[test]
fn spinner_images_are_cached_per_frame_and_wrap() {
    let images = Images::new();
    let a = spinner::image(&images, 32, 3, Weight::Regular);
    assert_eq!(spinner::image(&images, 32, 3 + SPINNER_FRAMES, Weight::Regular), a);
    assert_ne!(spinner::image(&images, 32, 4, Weight::Regular), a);
    assert_eq!(images.len(), 2);
}
