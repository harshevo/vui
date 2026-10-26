//! vui icons: a set of line icons on a 24 by 24 grid, drawn by vui's own rasteriser at any size and
//! weight into masks that the renderer tints. Icons are SVG path data, so adding one is a few lines.
//!
//! ```
//! use vui_icons::{Weight, set};
//!
//! let mask = set::SEARCH.render(32, Weight::Regular);
//! assert_eq!((mask.width, mask.height), (32, 32));
//! assert!(mask.ink() > 20.0);
//! ```

mod icon;
pub mod path;
pub mod raster;
pub mod set;
pub mod spinner;

pub use icon::{GRID, Icon, IconData, Kind, Layer, Weight};
pub use raster::Mask;
pub use spinner::SPINNER_FRAMES;

/// The built-in icon with this name, such as `"chevron-down"`.
pub fn by_name(name: &str) -> Option<Icon> {
    set::ALL.iter().copied().find(|icon| icon.name() == name)
}
