//! Running the gallery without a window: for the screenshot bin and the golden tests.

use vui::core::{Resources, size};
use vui::render::Image;
use vui::render::testing::render_with;
use vui::runtime::Ui;
use vui::runtime::testing::TestUi;

use crate::{Gallery, Msg, Page};

/// How wide the gallery window is: a 240 sidebar, a hairline and a 799 point page.
pub const WINDOW_WIDTH: f32 = 1040.0;

/// The shortest window: tall enough for the whole sidebar.
pub const MIN_HEIGHT: f32 = 560.0;

/// Taller than any page, so that a page is laid out whole before the window is cut to fit it.
const PROBE_HEIGHT: f32 = 8192.0;

/// The gallery on `page` in a window that shows all of it, driven by hand.
pub struct Headless {
    pub ui: TestUi<Msg>,
    pub res: Resources,
}

impl Headless {
    pub fn new(page: Page, dark: bool, scale: f32) -> Headless {
        Headless::with_width(page, dark, scale, WINDOW_WIDTH)
    }

    /// The gallery in a window `width` wide: below 600 it is laid out for a phone-sized window.
    pub fn with_width(page: Page, dark: bool, scale: f32, width: f32) -> Headless {
        let res = Resources::bundled_only();
        let mut gallery = Gallery::new(&res, page, dark);
        gallery.still = true;
        let mut ui = TestUi { ui: Ui::with_app(res.clone(), size(width, PROBE_HEIGHT), scale, gallery) };
        // A page is as tall as what is on it, so ask the page and cut the window to fit.
        let height = ui.rect("page").bottom().max(MIN_HEIGHT);
        ui.ui.set_size(size(width, height));
        // Let the first frame's springs come to rest: the screenshot is taken at a fixed time.
        ui.advance(0.6);
        Headless { ui, res }
    }

    /// What is on screen now, rendered on the GPU.
    pub fn image(&mut self) -> Image {
        let scene = self.ui.scene();
        render_with(&scene, &self.res)
    }

    pub fn gallery(&self) -> &Gallery {
        self.ui.app::<Gallery>()
    }
}
