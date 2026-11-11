//! Typing: text fields, search and password fields, text areas, number inputs, and the editing
//! model under them.

pub mod editor;
mod number_input;
mod text_field;

pub use number_input::number_input;
pub use text_field::{FieldState, TextField, TextFieldWidget, password_field, search_field, text_area, text_field};
