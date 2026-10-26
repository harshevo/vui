//! The heart of vui: everything that does not need a GPU or a window.
//!
//! Geometry, colour, theme tokens, springs and tweens, input events, text shaping, and the widget
//! runtime that lays out, routes events and paints into a [`scene::Scene`]. The renderer
//! (`vui-render`) and the platform backends turn that scene and those events into pixels and
//! input, so all of this runs headless in tests.

pub mod anim;
pub mod color;
pub mod geom;
pub mod input;
pub mod scene;
pub mod theme;

pub use anim::{Easing, Lerp, Spring, SpringValue, Tween};
pub use color::Color;
pub use geom::{Corners, Insets, Point, Rect, Size, Vec2, pt, rect, size, vec2};
pub use input::{CursorIcon, Event, ImeEvent, Key, KeyEvent, Modifiers, NamedKey, PointerButton, ScrollPhase};
pub use scene::{Border, Command, Quad, Scene};
pub use theme::{Mode, Palette, TextRole, Theme, space};
