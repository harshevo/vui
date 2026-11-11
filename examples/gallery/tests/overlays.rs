//! The overlays page with each overlay open, light and dark. Re-bless after an intended change and
//! look first:
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p gallery --test overlays
//! ```

use gallery::{Gallery, Headless, Overlay, Page};
use vui::core::pt;
use vui::render::assert_golden;

fn open(dark: bool, f: impl FnOnce(&mut Gallery)) -> Headless {
    let mut h = Headless::new(Page::Overlays, dark, 1.0);
    h.ui.update_app(f);
    h.ui.settle();
    h
}

#[test]
fn menu_open_light() {
    assert_golden!("overlay_menu_light", open(false, |g| g.overlay = Some(Overlay::Menu)).image());
}

#[test]
fn menu_open_dark() {
    assert_golden!("overlay_menu_dark", open(true, |g| g.overlay = Some(Overlay::Menu)).image());
}

#[test]
fn select_open_light() {
    assert_golden!("overlay_select_light", open(false, |g| g.overlay = Some(Overlay::Select)).image());
}

#[test]
fn context_menu_dark() {
    assert_golden!("overlay_context_dark", open(true, |g| g.context = Some(pt(420.0, 470.0))).image());
}

#[test]
fn dialog_light() {
    assert_golden!("overlay_dialog_light", open(false, |g| g.overlay = Some(Overlay::Dialog)).image());
}

#[test]
fn dialog_dark() {
    assert_golden!("overlay_dialog_dark", open(true, |g| g.overlay = Some(Overlay::Dialog)).image());
}

#[test]
fn toasts_and_a_tooltip_light() {
    let mut h = open(false, |g| {
        g.toasts = vec![1, 2];
        g.toasted = 2;
    });
    h.ui.move_to_key("copy-tip");
    h.ui.advance(0.8);
    assert_golden!("overlay_toast_tooltip_light", h.image());
}
