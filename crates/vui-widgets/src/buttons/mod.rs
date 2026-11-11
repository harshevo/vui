//! Things to press: buttons in every variant and size, and segmented controls.

pub(crate) mod button;
mod segmented;

pub use button::{Button, ButtonSize, ButtonVariant, button, icon_button};
pub use segmented::{segmented, segmented_keyed};
