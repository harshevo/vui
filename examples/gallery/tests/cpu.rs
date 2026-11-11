//! The CPU renderer against the GPU: every page of the gallery drawn in memory must match the GPU
//! goldens to within a few levels per channel. These never bless; the goldens belong to the GPU.

use std::path::PathBuf;

use gallery::{Headless, Page};
use vui::render::Image;
use vui::render::testing::render_cpu;

/// Channels within this distance count as equal.
const TOLERANCE: u8 = 2;
/// Fraction of pixels allowed past it: picture filtering differs a little from the GPU sampler.
const MAX_BAD: f32 = 0.001;

fn compare(page: Page, dark: bool, scale: f32) {
    let theme = if dark { "dark" } else { "light" };
    let suffix = if scale == 1.0 { String::new() } else { format!("_{scale}x") };
    let name = format!("{}_{theme}{suffix}", page.name().to_lowercase());
    let mut shot = Headless::new(page, dark, scale);
    let scene = shot.ui.scene();
    let actual = render_cpu(&scene, &shot.res);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let expected = Image::load_png(&root.join("golden").join(format!("{name}.png"))).expect("the GPU golden");
    let diff = actual.diff(&expected, TOLERANCE);
    let allowed = (diff.total_pixels as f32 * MAX_BAD).ceil() as usize;
    eprintln!("{name}: {} of {} pixels over {TOLERANCE}, max {}", diff.differing_pixels, diff.total_pixels, diff.max_channel_diff);
    if diff.size_mismatch || diff.differing_pixels > allowed {
        let out = root.join("actual");
        actual.save_png(&out.join(format!("cpu_{name}.png"))).expect("write actual");
        actual.diff_image(&expected, TOLERANCE).save_png(&out.join(format!("cpu_{name}.diff.png"))).expect("write diff");
        panic!("the CPU frame of `{name}` differs from the GPU golden: {} of {} pixels (max channel diff {}); see tests/actual/cpu_{name}.png", diff.differing_pixels, diff.total_pixels, diff.max_channel_diff);
    }
}

macro_rules! pages {
    ($($module:ident => $page:ident),* $(,)?) => {
        $(
            mod $module {
                use super::*;

                #[test]
                fn light() {
                    compare(Page::$page, false, 1.0);
                }

                #[test]
                fn dark() {
                    compare(Page::$page, true, 1.0);
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

#[test]
fn at_double_scale() {
    compare(Page::Foundations, false, 2.0);
    compare(Page::Controls, true, 2.0);
}
