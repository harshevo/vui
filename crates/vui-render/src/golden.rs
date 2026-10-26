//! Golden-image testing.
//!
//! A test renders a scene headless and calls `assert_golden!("name", image)`. The first time, or after
//! an intended visual change, run with `VUI_BLESS=1` to write `tests/golden/name.png`, then open the
//! file and look at it. On a mismatch the actual frame and a highlighted diff are written to
//! `tests/actual/` so the failure can be read by eye.

use std::path::PathBuf;

use crate::image::Image;

#[derive(Copy, Clone, Debug)]
pub struct GoldenOptions {
    /// Channels within this distance count as equal (absorbs GPU and driver rounding).
    pub channel_tolerance: u8,
    /// Fraction of pixels allowed to exceed the tolerance.
    pub max_bad_fraction: f32,
}

impl Default for GoldenOptions {
    fn default() -> GoldenOptions {
        GoldenOptions { channel_tolerance: 2, max_bad_fraction: 0.001 }
    }
}

/// Compares `actual` with `tests/golden/<name>.png` under `manifest_dir`. Panics on mismatch.
pub fn assert_golden(manifest_dir: &str, name: &str, actual: &Image, opts: &GoldenOptions) {
    let root = PathBuf::from(manifest_dir).join("tests");
    let golden_path = root.join("golden").join(format!("{name}.png"));
    let actual_path = root.join("actual").join(format!("{name}.png"));

    if std::env::var_os("VUI_BLESS").is_some() {
        actual.save_png(&golden_path).expect("write golden");
        eprintln!("blessed {}", golden_path.display());
        return;
    }

    let Ok(expected) = Image::load_png(&golden_path) else {
        actual.save_png(&actual_path).expect("write actual");
        panic!(
            "golden image `{name}` is missing. Look at {} and, if it is right, run with VUI_BLESS=1 to keep it.",
            actual_path.display()
        );
    };

    let diff = actual.diff(&expected, opts.channel_tolerance);
    let allowed = (diff.total_pixels as f32 * opts.max_bad_fraction).ceil() as usize;
    if diff.size_mismatch || diff.differing_pixels > allowed {
        actual.save_png(&actual_path).expect("write actual");
        let diff_path = root.join("actual").join(format!("{name}.diff.png"));
        actual.diff_image(&expected, opts.channel_tolerance).save_png(&diff_path).expect("write diff");
        panic!(
            "golden image `{name}` differs: {} of {} pixels over tolerance (max channel diff {}), size mismatch: {}.\n  actual: {}\n  diff:   {}\n  If the change is intended, run with VUI_BLESS=1.",
            diff.differing_pixels,
            diff.total_pixels,
            diff.max_channel_diff,
            diff.size_mismatch,
            actual_path.display(),
            diff_path.display()
        );
    }
}

/// `assert_golden!("name", image)` using the calling crate's `tests/golden` directory.
#[macro_export]
macro_rules! assert_golden {
    ($name:expr, $image:expr) => {
        $crate::golden::assert_golden(env!("CARGO_MANIFEST_DIR"), $name, &$image, &$crate::golden::GoldenOptions::default())
    };
    ($name:expr, $image:expr, $opts:expr) => {
        $crate::golden::assert_golden(env!("CARGO_MANIFEST_DIR"), $name, &$image, &$opts)
    };
}
