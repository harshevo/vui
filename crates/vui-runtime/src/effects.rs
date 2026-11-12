//! What the runtime asks of the platform, and what the platform gives it.

use std::cell::RefCell;

use vui_core::{CursorIcon, Rect};

use crate::platform::{FileDialog, MenuSpec, Notification};

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
    /// Ask for files or folders to open; answer with `Ui::answer(id, paths)`, empty if cancelled.
    Open { id: u64, dialog: FileDialog },
    /// Ask where to save; answer with `Ui::answer(id, paths)`, one path or none if cancelled.
    Save { id: u64, dialog: FileDialog },
    /// Show a notification. Answer with `Ui::answer(id, NotificationResponse::..)` when the person
    /// clicks it, presses one of its actions or dismisses it.
    Notify { id: u64, notification: Notification },
    /// Go full screen (true), leave it (false), or switch (None).
    FullScreen(Option<bool>),
    /// The app's own menus, to put in the menu bar in place of the ones before. Report choices
    /// with `Ui::choose_menu(id)`.
    Menus(Vec<MenuSpec>),
    /// The app's menu-bar item, or none.
    StatusItem(Option<MenuSpec>),
    /// The native views the window shows now, in paint order: create the new ones, move the others
    /// and remove the ones not listed. Sent when the list changes.
    Embeds(Vec<vui_core::Embed>),
}

/// Clipboard access: text both ways, and a picture to paste. The backend installs the system
/// pasteboard; tests use [`MemoryClipboard`].
pub trait Clipboard {
    fn get(&self) -> Option<String>;
    fn set(&self, text: &str);
    /// A picture on the clipboard, as PNG bytes.
    fn image(&self) -> Option<Vec<u8>> {
        None
    }
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
