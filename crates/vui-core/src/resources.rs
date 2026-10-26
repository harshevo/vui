//! The shared, single-threaded resources a UI and its renderer both need.

use crate::image::Images;
use crate::text::Fonts;

/// Fonts and images. Cheap to clone; clones share everything. The `Ui` shapes text and registers
/// images through it, and the renderer reads the very same objects when it draws.
#[derive(Clone, Default)]
pub struct Resources {
    pub fonts: Fonts,
    pub images: Images,
}

impl Resources {
    pub fn new() -> Resources {
        Resources::default()
    }

    /// Only the bundled fonts, so rendering is identical on every machine.
    pub fn bundled_only() -> Resources {
        Resources { fonts: Fonts::bundled_only(), images: Images::new() }
    }
}
