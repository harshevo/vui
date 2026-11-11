//! Every page of the gallery, light and dark, kept as a golden screenshot.
//!
//! The pages are shot at scale 1 to keep the files small, and two of them again at scale 2, where
//! text and hairlines land on different device pixels. When a change is meant to alter how things
//! look, re-bless and open the new files before committing them:
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p gallery --test golden
//! ```

use gallery::{Headless, Page};
use vui::render::assert_golden;

fn shoot(page: Page, dark: bool, scale: f32) {
    let theme = if dark { "dark" } else { "light" };
    let suffix = if scale == 1.0 { String::new() } else { format!("_{scale}x") };
    let name = format!("{}_{theme}{suffix}", page.name().to_lowercase());
    let image = Headless::new(page, dark, scale).image();
    assert_golden!(&name, image);
}

/// A module per page with a test for each theme, so one failure never hides another.
macro_rules! pages {
    ($($module:ident => $page:ident),* $(,)?) => {
        $(
            mod $module {
                use super::*;

                #[test]
                fn light() {
                    shoot(Page::$page, false, 1.0);
                }

                #[test]
                fn dark() {
                    shoot(Page::$page, true, 1.0);
                }
            }
        )*
    };
}

pages! {
    foundations => Foundations,
    buttons => Buttons,
    controls => Controls,
    inputs => Inputs,
    lists => Lists,
    images => Images,
    icons => Icons,
    overlays => Overlays,
    data => Data,
    feedback => Feedback,
    settings => Settings,
}

mod at_double_scale {
    use super::*;

    #[test]
    fn foundations_light() {
        shoot(Page::Foundations, false, 2.0);
    }

    #[test]
    fn controls_dark() {
        shoot(Page::Controls, true, 2.0);
    }
}
