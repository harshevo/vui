//! The editing model, driven the way the keyboard, the pointer and input methods drive it.

use vui_core::{Fonts, TextStyle, Theme};

use super::*;

fn ed(text: &str, anchor: usize, head: usize) -> Editor {
    let mut e = Editor::new();
    e.set_multiline(true);
    e.set_text(text);
    e.set_selection(anchor, head);
    e
}

fn caret(text: &str, at: usize) -> Editor {
    ed(text, at, at)
}

fn layout(text: &str, width: Option<f32>) -> TextLayout {
    let fonts = Fonts::bundled_only();
    fonts.layout(text, &TextStyle::from_token(Theme::light().types.body), width)
}

mod composition;
mod editing;
mod history;
mod moving;
