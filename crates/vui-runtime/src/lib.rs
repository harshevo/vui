//! The vui widget runtime: elements, reconciliation, layout, input routing, focus, scrolling,
//! animation and accessibility. It produces a [`vui_core::Scene`] and consumes
//! [`vui_core::Event`]s, so all of it runs headless in tests.

pub mod access;
mod access_tree;
mod app;
pub mod builtin;
pub mod effects;
pub mod element;
pub mod key;
pub mod layout;
mod node;
mod paint;
pub mod responsive;
pub mod style;
pub mod testing;
pub mod ui;
pub mod widget;

pub use app::{App, AppSender, Cx, Waker};
pub use access::{A11y, AccessAction, AccessActionKind, AccessInfo, AccessNode, AccessRange, AccessTree, Role};
pub use builtin::{Img, Label, img, label, small, title};
pub use effects::{Clipboard, Effect, MemoryClipboard};
pub use element::{
    Base, Builder, Click, Container, Decor, Div, Element, FocusGroup, IntoChildren, IntoElement, Parent, Radius, VirtualList, column, div, grid, row,
    spacer, virtual_list,
};
pub use key::Key;
pub use responsive::{Breakpoint, Restyle, Width};
pub use style::{
    AUTO, Align, Axis, Columns, Direction, Edges, Grid, Justify, Len, Overflow, Position, Sides, Style, Styled, Thumb, Track, fr, pct, px,
};
pub use ui::{Inspect, Ui};
pub use widget::{Dirty, Env, EventCx, Focus, FocusBy, PaintCx, TickCx, Widget, WidgetEvent};
