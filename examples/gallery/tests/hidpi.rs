//! High-density displays. A window moved to a screen with another scale is handed the new scale and
//! must draw exactly what a window opened there would; and phones' 3x screens get a golden of their
//! own, where text, hairlines and icons land on a third of a point.
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p gallery --test hidpi
//! ```

use gallery::{Headless, Page};
use vui::render::assert_golden;

const PHONE: f32 = 390.0;

#[test]
fn moving_to_a_denser_screen_draws_what_opening_there_would() {
    for page in [Page::Foundations, Page::Controls, Page::Charts] {
        let mut moved = Headless::new(page, false, 1.0);
        moved.image();
        moved.ui.ui.set_scale(2.0);
        let mut fresh = Headless::new(page, false, 2.0);
        let (a, b) = (moved.image(), fresh.image());
        let diff = a.diff(&b, 0);
        assert!(!diff.size_mismatch && diff.differing_pixels == 0, "{}: {diff:?}", page.name());

        // And back to a standard screen.
        moved.ui.ui.set_scale(1.0);
        let diff = moved.image().diff(&Headless::new(page, false, 1.0).image(), 0);
        assert_eq!(diff.differing_pixels, 0, "{} back at 1x", page.name());
    }
}

#[test]
fn controls_on_a_3x_phone() {
    let mut h = Headless::with_width(Page::Controls, false, 3.0, PHONE);
    h.ui.settle();
    assert_golden!("compact_controls_light_3x", h.image());
}

#[test]
fn effects_on_a_3x_phone_in_dark() {
    // Shaded fills at a third of a point. (The charts page is too tall for one 3x texture.)
    let mut h = Headless::with_width(Page::Effects, true, 3.0, PHONE);
    h.ui.settle();
    assert_golden!("compact_effects_dark_3x", h.image());
}
