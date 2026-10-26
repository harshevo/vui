//! Platform-independent input.
//!
//! Backends translate their native events into these types, so the widget runtime, the tests and the
//! gallery never touch AppKit. Positions are in logical pixels relative to the window's content area.

use std::path::PathBuf;

use crate::geom::{Point, Vec2};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Command on macOS, the Windows key elsewhere.
    pub cmd: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { shift: false, ctrl: false, alt: false, cmd: false };
    pub const SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };
    pub const CTRL: Modifiers = Modifiers { ctrl: true, ..Modifiers::NONE };
    pub const ALT: Modifiers = Modifiers { alt: true, ..Modifiers::NONE };
    pub const CMD: Modifiers = Modifiers { cmd: true, ..Modifiers::NONE };

    pub fn any(self) -> bool {
        self.shift || self.ctrl || self.alt || self.cmd
    }

    /// The platform's shortcut modifier: Command on macOS.
    pub fn shortcut(self) -> bool {
        self.cmd
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u8),
}

/// Where a scroll gesture is. Trackpads report a began/changed/ended sequence, then the system keeps
/// sending momentum events after the fingers lift.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ScrollPhase {
    /// A wheel tick with no gesture.
    #[default]
    None,
    Began,
    Changed,
    Ended,
    Cancelled,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Space,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Named(NamedKey),
    /// The key's unshifted character, lower case, for shortcuts such as Cmd-A.
    Character(char),
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Modifiers,
    /// Text the key produced, if any. Empty for navigation keys and for shortcuts.
    pub text: Option<String>,
    pub pressed: bool,
    pub repeat: bool,
}

impl KeyEvent {
    pub fn press(key: Key, mods: Modifiers) -> KeyEvent {
        KeyEvent { key, mods, text: None, pressed: true, repeat: false }
    }

    pub fn named(key: NamedKey) -> KeyEvent {
        KeyEvent::press(Key::Named(key), Modifiers::NONE)
    }

    pub fn character(c: char) -> KeyEvent {
        KeyEvent { key: Key::Character(c.to_ascii_lowercase()), mods: Modifiers::NONE, text: Some(c.to_string()), pressed: true, repeat: false }
    }

    pub fn shortcut(c: char) -> KeyEvent {
        KeyEvent::press(Key::Character(c), Modifiers::CMD)
    }

    pub fn with_mods(mut self, mods: Modifiers) -> KeyEvent {
        self.mods = mods;
        self
    }
}

/// Input method composition, for Japanese, Chinese, Korean, dictation and the emoji picker.
#[derive(Clone, Debug, PartialEq)]
pub enum ImeEvent {
    /// Text being composed, shown inline and underlined. `cursor` is a byte range inside `text`.
    Preedit { text: String, cursor: Option<(usize, usize)> },
    /// Composition finished: insert this text.
    Commit(String),
    /// Composition cancelled.
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    PointerMoved { pos: Point, mods: Modifiers },
    PointerDown { pos: Point, button: PointerButton, mods: Modifiers, click_count: u8 },
    PointerUp { pos: Point, button: PointerButton, mods: Modifiers },
    /// The pointer left the window.
    PointerLeft,
    /// `delta` is how far the content should move, in logical pixels, with the user's natural-scroll
    /// setting already applied: positive `y` moves content down.
    Scroll { pos: Point, delta: Vec2, precise: bool, phase: ScrollPhase, momentum: bool },
    /// Pinch. `delta` is the change in scale: 0.1 means 10% larger.
    Magnify { pos: Point, delta: f32, phase: ScrollPhase },
    Key(KeyEvent),
    Ime(ImeEvent),
    /// The window became or stopped being the key window.
    WindowFocus(bool),
    /// Files are being dragged over the window.
    DragOver { pos: Point, paths: Vec<PathBuf> },
    DragLeft,
    Drop { pos: Point, paths: Vec<PathBuf> },
}

/// The pointer shape a widget asks for while hovered.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    #[default]
    Arrow,
    Pointer,
    Text,
    Grab,
    Grabbing,
    ResizeHorizontal,
    ResizeVertical,
    NotAllowed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_helpers() {
        assert!(!Modifiers::NONE.any());
        assert!(Modifiers::CMD.shortcut());
        assert!(Modifiers::SHIFT.any());
        assert!(!Modifiers::CTRL.shortcut());
    }

    #[test]
    fn key_event_builders() {
        let e = KeyEvent::character('A');
        assert_eq!(e.key, Key::Character('a'));
        assert_eq!(e.text.as_deref(), Some("A"));
        let s = KeyEvent::shortcut('a');
        assert!(s.mods.cmd && s.text.is_none());
        assert_eq!(KeyEvent::named(NamedKey::Enter).key, Key::Named(NamedKey::Enter));
        assert!(KeyEvent::named(NamedKey::Tab).with_mods(Modifiers::SHIFT).mods.shift);
    }
}
