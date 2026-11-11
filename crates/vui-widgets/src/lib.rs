//! vui's stock widgets: buttons, switches, checkboxes, sliders, text fields, lists, the sidebar and the
//! rest of what a window is made of, all built on `vui-runtime`.
//!
//! Every widget is a function that returns a builder: `button("Save").primary().on_click(Msg::Save)`.
//! The builders have the whole element vocabulary of `vui-runtime` (size, padding, keys, handlers,
//! accessibility) as well as their own methods. They are controlled: a widget shows the state the app
//! gives it and sends messages for the state the user asks for.
//!
//! The widgets are grouped by what they are for, and every one is re-exported here:
//!
//! | Module | Widgets |
//! | --- | --- |
//! | `buttons` | `button`, `icon_button`, `segmented` |
//! | `controls` | `checkbox`, `radio`, `radio_group`, `toggle`, `slider`, `range_slider` |
//! | `inputs` | `text_field`, `search_field`, `password_field`, `text_area`, `number_input` |
//! | `display` | `icon`, `card`, `badge`, `avatar`, `kbd`, `separator`, `chip`, `image_tile`, `markdown`, `rich_text`, `glow`, `shimmer`, `aurora`, `border_beam`, `spotlight` |
//! | `data` | `item`, `data_table`, `tree`, `lasso` |
//! | `feedback` | `progress`, `spinner`, `alert`, `skeleton`, `empty_state` |
//! | `layout` | `field`, `setting`, `splitter`, `disclosure`, `accordion` |
//! | `navigation` | `sidebar`, `nav_item`, `list`, `list_row`, `tabs`, `breadcrumbs`, `pagination` |
//! | `overlay` | `popover`, `dropdown`, `menu`, `select`, `context_menu`, `dialog`, `toasts` |

mod buttons;
mod controls;
mod data;
mod display;
mod feedback;
mod inputs;
mod layout;
mod navigation;
pub mod overlay;

pub use buttons::*;
pub use controls::*;
pub use data::*;
pub use display::*;
pub use feedback::*;
pub use inputs::*;
pub use layout::*;
pub use navigation::*;
pub use overlay::{
    Popover, Side, context_menu, dialog, dialog_actions, dropdown, menu, menu_check, menu_heading, menu_item, menu_separator, panel, popover,
    select, toast, toasts,
};
