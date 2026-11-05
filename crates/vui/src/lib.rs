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
//!     fn update(&mut self, msg: Msg, _cx: &mut Cx) {
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

pub use vui_core as core;
pub use vui_icons as icons;
pub use vui_render as render;
pub use vui_runtime as runtime;
pub use vui_widgets as widgets;

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
}
