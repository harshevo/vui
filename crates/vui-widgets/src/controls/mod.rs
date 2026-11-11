//! Choosing values: checkboxes, radios, switches and sliders.

mod choice;
mod slider;

pub use choice::{CheckState, Checkbox, ControlWidget, Radio, Toggle, checkbox, radio, radio_group, toggle};
pub use slider::{Slider, SliderWidget, slider};
