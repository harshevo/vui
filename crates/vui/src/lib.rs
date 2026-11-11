//! vui: a GPU-drawn UI framework. This crate is the front door: it re-exports the pieces, and
//! [`prelude`] has what most windows need.
//!
//! ```
//! use vui::prelude::*;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Save,
//! }
//!
//! struct Notes {
//!     saved: bool,
//! }
//!
//! impl App for Notes {
//!     type Msg = Msg;
//!
//!     fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
//!         match msg {
//!             Msg::Save => self.saved = true,
//!         }
//!     }
//!
//!     fn view(&self) -> impl IntoElement<Msg> {
//!         column().padding(24.0).gap(12.0).align_items(Align::Start).children((
//!             title("Notes"),
//!             button(if self.saved { "Saved" } else { "Save" }).primary().on_click(Msg::Save),
//!         ))
//!     }
//! }
//! ```

//!
//! Open it in a window with [`run`]:
//!
//! ```no_run
//! # use vui::prelude::*;
//! # struct Notes;
//! # impl App for Notes {
//! #     type Msg = ();
//! #     fn update(&mut self, _: (), _: &mut Cx<()>) {}
//! #     fn view(&self) -> impl IntoElement<()> { label("Notes") }
//! # }
//! # #[cfg(target_os = "macos")]
//! vui::run(WindowOptions::new("Notes").size(size(480.0, 360.0)), |_resources| Notes).unwrap();
//! ```
//!
//! | Module | What it holds |
//! | --- | --- |
//! | [`core`] | geometry, colour, the theme, springs, input events, text and the scene |
//! | [`runtime`] | elements, layout, styling, input, focus, animation and accessibility |
//! | [`widgets`] | the standard widgets |
//! | [`icons`] | the icon set |
//! | [`viz`] | charts, sparklines and graphs |
//! | [`render`] | the GPU renderer and its CPU fallback |
//! | [`testing`] | driving a UI and comparing frames, with no window |
//! | [`platform`] | the window backend (macOS) |

pub use vui_core as core;
pub use vui_icons as icons;
pub use vui_render as render;
pub use vui_runtime as runtime;
pub use vui_viz as viz;
pub use vui_widgets as widgets;

/// The window backend: windows, see-through overlays, a `Proxy` per window for other threads.
#[cfg(target_os = "macos")]
pub use vui_macos as platform;
#[cfg(target_os = "macos")]
pub use vui_macos::{Application, Error, OverlayOptions, Place, Proxy, Screen, WindowOptions, run};

/// Testing without a window: [`TestUi`](testing::TestUi) clicks, types and moves time by hand, and
/// the render helpers draw a frame on the GPU, or the CPU where there is none, to compare with a
/// golden image (`assert_golden!`).
pub mod testing {
    pub use vui_render::assert_golden;
    pub use vui_render::testing::{render_cpu, render_scene, render_with, uses_cpu};
    pub use vui_runtime::testing::*;
}

/// The names most windows use, in one import: `use vui::prelude::*;`.
///
/// Two names are left out because both crates have one: the keyboard `Key` of `vui_core` and the
/// element `Key` of `vui_runtime`. Spell them `core::Key` and `runtime::Key`.
pub mod prelude {
    pub use vui_core::{
        Color, Corners, CursorIcon, Fill, Insets, Mode, NamedKey, Point, Rect, Shadow, Size, Spring, Theme, Tone, pt, rect, size,
    };
    pub use vui_icons::{Icon, Weight, set as icon_set};
    pub use vui_runtime::{
        AUTO, A11y, AccessAction, App, Align, Breakpoint, Builder, Click, Cx, Div, Element, Focus, FocusGroup, IntoChildren, IntoElement, Justify,
        Img, Label, Overflow, Parent, Radius, Role, Styled, Width, column, div, img, label, pct, px, row, small, spacer, title,
    };
    pub use vui_widgets::*;
    #[cfg(target_os = "macos")]
    pub use vui_macos::{Proxy, WindowOptions};
}

/// The guides in `docs/`, compiled as doctests so that their code stays true to the API.
#[cfg(all(doctest, target_os = "macos"))]
mod guides {
    macro_rules! guides {
        ($($module:ident => $file:literal),* $(,)?) => {
            $(
                #[doc = include_str!(concat!("../../../docs/", $file))]
                mod $module {}
            )*
        };
    }

    guides! {
        getting_started => "getting-started.md",
        app_model => "app-model.md",
        layout => "layout.md",
        styling => "styling.md",
        widgets => "widgets.md",
        text_icons_images => "text-icons-images.md",
        charts => "charts.md",
        custom_widgets => "custom-widgets.md",
        rendering => "rendering.md",
        testing => "testing.md",
        accessibility => "accessibility.md",
    }
}
