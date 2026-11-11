//! vui's stock widgets: buttons, switches, checkboxes, sliders, text fields, lists, the sidebar and the
//! rest of what a window is made of, all built on `vui-runtime`.
//!
//! Every widget is a function that returns a builder: `button("Save").primary().on_click(Msg::Save)`.
//! The builders have the whole element vocabulary of `vui-runtime` (size, padding, keys, handlers,
//! accessibility) as well as their own methods. They are controlled: a widget shows the state the app
//! gives it and sends messages for the state the user asks for.

mod button;
mod control;
mod data;
mod display;
mod feedback;
mod inputs;
mod layout;
mod navigation;
pub mod editor;
mod form;
mod icon;
mod item;
mod progress;
mod effects;
mod lasso;
pub mod markdown;
pub mod overlay;
mod segmented;
mod sidebar;
mod slider;
mod splitter;
mod text_field;
mod tile;

pub use button::{Button, ButtonSize, ButtonVariant, button, icon_button};
pub use control::{CheckState, Checkbox, ControlWidget, Radio, Toggle, checkbox, radio, radio_group, toggle};
pub use display::{
    Badge, BadgeTint, avatar, avatar_image, avatar_sized, badge, card, dot, kbd, section_title, separator, separator_vertical,
};
pub use data::{ColWidth, SortDir, TableColumn, TableRow, TreeRow, data_table, table_column, table_row, tree, tree_row};
pub use display::chip;
pub use feedback::{AlertKind, alert, alert_with, empty_state, skeleton, skeleton_text};
pub use form::{field, setting};
pub use inputs::number_input;
pub use layout::{accordion, disclosure};
pub use navigation::{breadcrumbs, pagination, tab_panel, tabs, tabs_keyed};
pub use icon::{ICON_SIZE, IconEl, IconWidget, SPINNER_PERIOD, icon, paint_icon, paint_spinner};
pub use item::{Item, ItemWidget, item};
pub use markdown::{Markdown, RichText, markdown, rich_text};
pub use lasso::{Lasso, LassoState, LassoWidget, bounds as lasso_bounds, lasso};
pub use effects::{EffectState, Glow, GlowWidget, Shimmer, ShimmerWidget, glow, shimmer};
pub use overlay::{
    Popover, Side, context_menu, dialog, dialog_actions, dropdown, menu, menu_check, menu_heading, menu_item, menu_separator, panel, popover,
    select, toast, toasts,
};
pub use progress::{Progress, ProgressWidget, Spinner, SpinnerWidget, progress, progress_indeterminate, spinner};
pub use segmented::{segmented, segmented_keyed};
pub use sidebar::{ListRow, NavItem, SELECTED, SIDEBAR_WIDTH, list, list_row, nav_heading, nav_item, nav_section, sidebar};
pub use slider::{Slider, SliderWidget, slider};
pub use splitter::{Splitter, SplitterWidget, splitter};
pub use text_field::{FieldState, TextField, TextFieldWidget, password_field, search_field, text_area, text_field};
pub use tile::{ImageTile, TileWidget, image_tile};
