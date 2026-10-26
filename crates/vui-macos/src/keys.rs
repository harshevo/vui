//! Keyboard translation: macOS virtual key codes and modifier flags to vui's keys.
//!
//! Kept free of AppKit types so the tables are tested on their own.

use vui_core::{Modifiers, NamedKey};

/// The keys that have a name rather than a character, by their virtual key code (the `kVK_` constants
/// of Carbon's `Events.h`). Codes are layout independent: they name a place on the keyboard.
pub fn named_key(code: u16) -> Option<NamedKey> {
    Some(match code {
        0x24 | 0x4c => NamedKey::Enter,
        0x30 => NamedKey::Tab,
        0x31 => NamedKey::Space,
        0x33 => NamedKey::Backspace,
        0x35 => NamedKey::Escape,
        0x75 => NamedKey::Delete,
        0x7b => NamedKey::ArrowLeft,
        0x7c => NamedKey::ArrowRight,
        0x7d => NamedKey::ArrowDown,
        0x7e => NamedKey::ArrowUp,
        0x73 => NamedKey::Home,
        0x77 => NamedKey::End,
        0x74 => NamedKey::PageUp,
        0x79 => NamedKey::PageDown,
        0x7a => NamedKey::F(1),
        0x78 => NamedKey::F(2),
        0x63 => NamedKey::F(3),
        0x76 => NamedKey::F(4),
        0x60 => NamedKey::F(5),
        0x61 => NamedKey::F(6),
        0x62 => NamedKey::F(7),
        0x64 => NamedKey::F(8),
        0x65 => NamedKey::F(9),
        0x6d => NamedKey::F(10),
        0x67 => NamedKey::F(11),
        0x6f => NamedKey::F(12),
        _ => return None,
    })
}

/// `NSEventModifierFlags` bits.
pub const SHIFT: usize = 1 << 17;
pub const CONTROL: usize = 1 << 18;
pub const OPTION: usize = 1 << 19;
pub const COMMAND: usize = 1 << 20;

pub fn modifiers(flags: usize) -> Modifiers {
    Modifiers { shift: flags & SHIFT != 0, ctrl: flags & CONTROL != 0, alt: flags & OPTION != 0, cmd: flags & COMMAND != 0 }
}

/// The character a key stands for in shortcuts: what it types with no modifiers, lower case. AppKit
/// hands over the unmodified characters; function keys arrive as characters in the private-use area,
/// which name nothing.
pub fn shortcut_char(unmodified: &str) -> Option<char> {
    let mut chars = unmodified.chars();
    let c = chars.next()?;
    if chars.next().is_some() || c.is_control() || ('\u{f700}'..='\u{f8ff}').contains(&c) {
        return None;
    }
    c.to_lowercase().next()
}

/// Text a key typed, if it is text: not a control character and not a function key.
pub fn typed_text(text: &str) -> Option<String> {
    let printable = !text.is_empty() && text.chars().all(|c| !c.is_control() && !('\u{f700}'..='\u{f8ff}').contains(&c));
    printable.then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_by_code() {
        assert_eq!(named_key(0x24), Some(NamedKey::Enter));
        assert_eq!(named_key(0x4c), Some(NamedKey::Enter), "the keypad's Enter too");
        assert_eq!(named_key(0x33), Some(NamedKey::Backspace), "the key labelled delete");
        assert_eq!(named_key(0x75), Some(NamedKey::Delete), "forward delete");
        assert_eq!(named_key(0x7e), Some(NamedKey::ArrowUp));
        assert_eq!(named_key(0x6f), Some(NamedKey::F(12)));
        assert_eq!(named_key(0x00), None, "A is a character");
    }

    #[test]
    fn modifier_flags() {
        assert_eq!(modifiers(0), Modifiers::NONE);
        assert_eq!(modifiers(COMMAND), Modifiers::CMD);
        let m = modifiers(SHIFT | OPTION | (1 << 16));
        assert!(m.shift && m.alt && !m.cmd && !m.ctrl, "caps lock is not a modifier");
    }

    #[test]
    fn shortcut_characters() {
        assert_eq!(shortcut_char("a"), Some('a'));
        assert_eq!(shortcut_char("A"), Some('a'));
        assert_eq!(shortcut_char("1"), Some('1'));
        assert_eq!(shortcut_char("\u{f700}"), None, "the up arrow's private character");
        assert_eq!(shortcut_char("\r"), None);
        assert_eq!(shortcut_char(""), None);
        assert_eq!(shortcut_char("ab"), None);
    }

    #[test]
    fn only_printable_text_is_typed() {
        assert_eq!(typed_text("é").as_deref(), Some("é"));
        assert_eq!(typed_text(" ").as_deref(), Some(" "));
        assert_eq!(typed_text("\u{1b}"), None);
        assert_eq!(typed_text("\u{f702}"), None);
        assert_eq!(typed_text(""), None);
    }
}
