//! Moving around an app: tabs, breadcrumbs and pagination.

mod breadcrumbs;
mod pagination;
mod tabs;

pub use breadcrumbs::breadcrumbs;
pub use pagination::pagination;
pub use tabs::{tab_panel, tabs, tabs_keyed};
