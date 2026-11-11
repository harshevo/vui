//! Pages on a phone-sized window, 390 points wide: the sidebar becomes a bar with a page menu, tables
//! drop their less important columns, pagination keeps only the current page, dialogs become sheets
//! and toasts run the width of the window. Re-bless after an intended change and look first:
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p gallery --test compact
//! ```

use gallery::{Gallery, Headless, Overlay, Page};
use vui::render::assert_golden;

const PHONE: f32 = 390.0;

fn shot(page: Page, dark: bool, f: impl FnOnce(&mut Gallery)) -> Headless {
    let mut h = Headless::with_width(page, dark, 1.0, PHONE);
    h.ui.update_app(f);
    h.ui.settle();
    h
}

#[test]
fn data_compact_light() {
    assert_golden!("compact_data_light", shot(Page::Data, false, |_| {}).image());
}

#[test]
fn feedback_compact_dark() {
    assert_golden!("compact_feedback_dark", shot(Page::Feedback, true, |_| {}).image());
}

#[test]
fn page_menu_compact_light() {
    assert_golden!("compact_nav_light", shot(Page::Controls, false, |g| g.nav_open = true).image());
}

#[test]
fn dialog_sheet_and_toasts_compact_light() {
    let h = shot(Page::Overlays, false, |g| {
        g.overlay = Some(Overlay::Dialog);
        g.toasts = vec![1];
    });
    let mut h = h;
    assert_golden!("compact_dialog_light", h.image());
}

#[test]
fn the_sidebar_gives_way_to_the_top_bar() {
    let mut h = Headless::with_width(Page::Buttons, false, 1.0, PHONE);
    assert_eq!(h.ui.rect("sidebar").w, 0.0, "the sidebar is hidden");
    assert!(h.ui.rect("top-bar").h > 0.0);
    let mut wide = Headless::new(Page::Buttons, false, 1.0);
    assert_eq!(wide.ui.rect("top-bar").h, 0.0);
    assert!(wide.ui.rect("sidebar").w > 0.0);
}
