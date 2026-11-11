//! Things that open over the page: popovers, menus, selects, context menus, dialogs and toasts.
//!
//! All of them are controlled: the app keeps whether each is open and gets a message when it should
//! close (a press outside it, Escape, or a choice). Each opens with a short spring and moves focus
//! into itself, so the keyboard works at once.

mod command_palette;
mod context_menu;
mod dialog;
mod drawer;
mod hover_card;
mod menu;
mod popover;
mod select;
mod toast;

pub use command_palette::{Command, command, command_palette};
pub use context_menu::context_menu;
pub use dialog::{dialog, dialog_actions};
pub use drawer::{Edge, drawer, sheet};
pub use hover_card::hover_card;
pub use menu::{menu, menu_check, menu_heading, menu_item, menu_separator};
pub use popover::{Popover, Side, dropdown, panel, popover};
pub use select::select;
pub use toast::{toast, toasts};
