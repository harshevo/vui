//! What a window or an overlay opens as.

use vui_core::{Size, size};

/// The window an app opens in.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowOptions {
    pub title: String,
    /// Content size in points.
    pub size: Size,
    pub min_size: Size,
    /// Quit when the window closes. Off for apps that live on in the menu bar.
    pub quit_on_close: bool,
}

impl WindowOptions {
    pub fn new(title: impl Into<String>) -> WindowOptions {
        WindowOptions { title: title.into(), size: size(960.0, 640.0), min_size: size(640.0, 420.0), quit_on_close: true }
    }

    pub fn size(mut self, size: Size) -> WindowOptions {
        self.size = size;
        self
    }

    pub fn min_size(mut self, size: Size) -> WindowOptions {
        self.min_size = size;
        self
    }

    pub fn quit_on_close(mut self, quit: bool) -> WindowOptions {
        self.quit_on_close = quit;
        self
    }
}

/// A borderless, see-through window that floats above everything, the menu bar included, on every
/// Space and over full-screen apps, and never takes focus. It sits where [`Place`] says on the screen
/// that `Application::screen` describes.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayOptions {
    /// Size in points. Whatever the app does not paint stays see-through.
    pub size: Size,
    /// Clicks pass through to whatever is underneath.
    pub click_through: bool,
    pub place: Place,
    /// Clicks pass through wherever the pointer is over nothing that takes it, and land on the overlay
    /// wherever it is: a panel that is mostly empty space.
    pub through_empty: bool,
    /// The overlay can take the keyboard when its app asks (`Cx::take_keys`), for a field typed into.
    /// It still never activates the app: the person's own app stays in front, and gets the keyboard
    /// back when the overlay's app lets go.
    pub keyboard: bool,
}

/// Where an overlay sits.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Place {
    /// At the top centre, its top edge at the top of the screen: over the notch.
    Top,
    /// Against the right edge, its top edge just under the menu bar.
    Right,
    /// Over the whole of display `n`, in the order `Application::displays` lists them.
    Display(usize),
}

impl OverlayOptions {
    pub fn new(size: Size) -> OverlayOptions {
        OverlayOptions { size, click_through: true, place: Place::Top, through_empty: false, keyboard: false }
    }

    pub fn click_through(mut self, through: bool) -> OverlayOptions {
        self.click_through = through;
        self
    }

    pub fn place(mut self, place: Place) -> OverlayOptions {
        self.place = place;
        self
    }

    /// See [`OverlayOptions::through_empty`].
    pub fn through_empty(mut self) -> OverlayOptions {
        self.through_empty = true;
        self
    }

    /// See [`OverlayOptions::keyboard`].
    pub fn keyboard(mut self) -> OverlayOptions {
        self.keyboard = true;
        self
    }
}

/// The screen overlays go on: the built-in display if it has a camera notch, else the one with the menu
/// bar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Screen {
    /// Size in points.
    pub size: Size,
    /// The camera housing's size in points, centred at the top, if the screen has one.
    pub notch: Option<Size>,
    /// Height of the menu bar in points (zero while it hides itself).
    pub menu_bar: f32,
}
