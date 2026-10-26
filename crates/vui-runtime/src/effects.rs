//! What the runtime asks of the platform, and what the platform gives it.

use std::cell::RefCell;

use vui_core::{CursorIcon, Rect};

/// Something the runtime wants the window system to do. The backend drains these after every event
/// and frame with `Ui::take_effects`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Effect {
    /// Show this pointer shape.
    Cursor(CursorIcon),
    /// Where the text caret is, in window coordinates, so the input method can place its candidate
    /// window. `None` means no text input has focus and the input method should stand down.
    ImeArea(Option<Rect>),
}

/// Plain-text clipboard access. The backend installs the system pasteboard; tests use
/// [`MemoryClipboard`].
pub trait Clipboard {
    fn get(&self) -> Option<String>;
    fn set(&self, text: &str);
}

/// A clipboard that lives in this process.
#[derive(Default)]
pub struct MemoryClipboard {
    text: RefCell<Option<String>>,
}

impl Clipboard for MemoryClipboard {
    fn get(&self) -> Option<String> {
        self.text.borrow().clone()
    }

    fn set(&self, text: &str) {
        *self.text.borrow_mut() = Some(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_round_trips() {
        let c = MemoryClipboard::default();
        assert_eq!(c.get(), None);
        c.set("hello");
        assert_eq!(c.get().as_deref(), Some("hello"));
        c.set("again");
        assert_eq!(c.get().as_deref(), Some("again"));
    }
}
