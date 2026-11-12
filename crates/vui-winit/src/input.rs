//! winit's input as vui's: keys, modifiers, buttons, scrolling, the input method, cursors and
//! clicks counted into double and triple clicks. Plain functions over plain values, so they are
//! tested without a window.

use std::time::{Duration, Instant};

use vui_core::{CursorIcon, ImeEvent, Key, KeyEvent, Modifiers, NamedKey, Point, PointerButton, ScrollPhase, Vec2, vec2};
use winit::event::{Ime, MouseButton, MouseScrollDelta, TouchPhase};
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey as WNamed};

/// Points one line of a mouse wheel scrolls.
pub const LINE: f32 = 16.0;

/// The keys vui names.
pub fn named_key(key: &WNamed) -> Option<NamedKey> {
    Some(match key {
        WNamed::Enter => NamedKey::Enter,
        WNamed::Tab => NamedKey::Tab,
        WNamed::Escape => NamedKey::Escape,
        WNamed::Backspace => NamedKey::Backspace,
        WNamed::Delete => NamedKey::Delete,
        WNamed::Space => NamedKey::Space,
        WNamed::ArrowLeft => NamedKey::ArrowLeft,
        WNamed::ArrowRight => NamedKey::ArrowRight,
        WNamed::ArrowUp => NamedKey::ArrowUp,
        WNamed::ArrowDown => NamedKey::ArrowDown,
        WNamed::Home => NamedKey::Home,
        WNamed::End => NamedKey::End,
        WNamed::PageUp => NamedKey::PageUp,
        WNamed::PageDown => NamedKey::PageDown,
        WNamed::F1 => NamedKey::F(1),
        WNamed::F2 => NamedKey::F(2),
        WNamed::F3 => NamedKey::F(3),
        WNamed::F4 => NamedKey::F(4),
        WNamed::F5 => NamedKey::F(5),
        WNamed::F6 => NamedKey::F(6),
        WNamed::F7 => NamedKey::F(7),
        WNamed::F8 => NamedKey::F(8),
        WNamed::F9 => NamedKey::F(9),
        WNamed::F10 => NamedKey::F(10),
        WNamed::F11 => NamedKey::F(11),
        WNamed::F12 => NamedKey::F(12),
        _ => return None,
    })
}

/// vui's modifiers. `cmd` is vui's shortcut modifier: Command on macOS, Ctrl on Windows and Linux,
/// so Ctrl-C copies there as Cmd-C does on a Mac. Off macOS, `ctrl` stays for the Ctrl key only
/// where it is not the shortcut key, which is nowhere, and the Windows or Super key is left to the
/// system.
pub fn modifiers(state: ModifiersState) -> Modifiers {
    let (shift, control, alt, logo) = (state.shift_key(), state.control_key(), state.alt_key(), state.super_key());
    if cfg!(target_os = "macos") {
        Modifiers { shift, ctrl: control, alt, cmd: logo }
    } else {
        Modifiers { shift, ctrl: false, alt, cmd: control }
    }
}

/// A key press or release as vui sees it. `unmodified` is the key with no modifiers held, where the
/// platform reports it, so Ctrl-Shift-Z still names `z`. Keys that are neither named nor characters
/// (Shift alone, media keys) give nothing.
pub fn key_event(logical: &WKey, unmodified: Option<&WKey>, text: Option<&str>, pressed: bool, repeat: bool, mods: Modifiers) -> Option<KeyEvent> {
    let key = match logical {
        WKey::Named(named) => Key::Named(named_key(named)?),
        WKey::Character(_) => {
            let c = [unmodified, Some(logical)].into_iter().flatten().find_map(|k| match k {
                WKey::Character(s) => s.chars().next(),
                _ => None,
            })?;
            Key::Character(c.to_lowercase().next().unwrap_or(c))
        }
        _ => return None,
    };
    let text = if pressed { text.and_then(|t| typed_text(t, mods)) } else { None };
    Some(KeyEvent { key, mods, text, pressed, repeat })
}

/// Text a key typed, if it is text: printable, and not a shortcut. AltGr arrives as Ctrl and Alt
/// together and types text, so Ctrl with Alt still types.
pub fn typed_text(text: &str, mods: Modifiers) -> Option<String> {
    let printable = !text.is_empty() && text.chars().all(|c| !c.is_control());
    let shortcut = mods.cmd && (cfg!(target_os = "macos") || !mods.alt);
    (printable && !shortcut).then(|| text.to_string())
}

pub fn button(b: MouseButton) -> PointerButton {
    match b {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Middle,
        MouseButton::Back => PointerButton::Other(3),
        MouseButton::Forward => PointerButton::Other(4),
        MouseButton::Other(n) => PointerButton::Other(n.min(255) as u8),
    }
}

/// How far content should move for a scroll, in points, whether it came from a precise device, and
/// its gesture phase. Wheels report lines; trackpads report pixels, which `scale` turns into points.
/// winit's signs already match vui's: positive `y` moves content down.
pub fn scroll(delta: MouseScrollDelta, phase: TouchPhase, scale: f32) -> (Vec2, bool, ScrollPhase) {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => (vec2(x * LINE, y * LINE), false, ScrollPhase::None),
        MouseScrollDelta::PixelDelta(p) => {
            let s = scale.max(0.1) as f64;
            (vec2((p.x / s) as f32, (p.y / s) as f32), true, gesture_phase(phase))
        }
    }
}

/// Where a trackpad gesture is: a scroll or a pinch.
pub fn gesture_phase(phase: TouchPhase) -> ScrollPhase {
    match phase {
        TouchPhase::Started => ScrollPhase::Began,
        TouchPhase::Moved => ScrollPhase::Changed,
        TouchPhase::Ended => ScrollPhase::Ended,
        TouchPhase::Cancelled => ScrollPhase::Cancelled,
    }
}

/// The input method's events as vui's. `composing` tracks whether text is being composed, so that
/// clearing or disabling the composition cancels it, and only then.
pub fn ime(event: Ime, composing: &mut bool) -> Option<ImeEvent> {
    match event {
        Ime::Enabled => None,
        Ime::Preedit(text, _) if text.is_empty() => std::mem::take(composing).then_some(ImeEvent::Cancel),
        Ime::Preedit(text, cursor) => {
            *composing = true;
            Some(ImeEvent::Preedit { text, cursor })
        }
        Ime::Commit(text) => {
            *composing = false;
            Some(ImeEvent::Commit(text))
        }
        Ime::Disabled => std::mem::take(composing).then_some(ImeEvent::Cancel),
    }
}

pub fn cursor(icon: CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match icon {
        CursorIcon::Arrow => W::Default,
        CursorIcon::Pointer => W::Pointer,
        CursorIcon::Text => W::Text,
        CursorIcon::Grab => W::Grab,
        CursorIcon::Grabbing => W::Grabbing,
        CursorIcon::ResizeHorizontal => W::EwResize,
        CursorIcon::ResizeVertical => W::NsResize,
        CursorIcon::NotAllowed => W::NotAllowed,
        CursorIcon::Crosshair => W::Crosshair,
    }
}

/// Counts presses of one button in one place into double and triple clicks: winit reports each
/// press on its own.
#[derive(Default)]
pub struct Clicks {
    last: Option<(Instant, Point, PointerButton)>,
    count: u8,
}

impl Clicks {
    /// Presses closer together than this count as one click more.
    pub const INTERVAL: Duration = Duration::from_millis(500);
    /// And no farther apart than this, in points.
    pub const SLOP: f32 = 4.0;

    pub fn press(&mut self, at: Instant, pos: Point, button: PointerButton) -> u8 {
        let again = self.last.is_some_and(|(t, p, b)| {
            b == button && at.saturating_duration_since(t) <= Clicks::INTERVAL && (p.x - pos.x).abs() <= Clicks::SLOP && (p.y - pos.y).abs() <= Clicks::SLOP
        });
        self.count = if again { self.count.saturating_add(1) } else { 1 };
        self.last = Some((at, pos, button));
        self.count
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;
    use winit::keyboard::SmolStr;

    use super::*;

    fn ch(s: &str) -> WKey {
        WKey::Character(SmolStr::new(s))
    }

    #[test]
    fn named_keys() {
        assert_eq!(named_key(&WNamed::Enter), Some(NamedKey::Enter));
        assert_eq!(named_key(&WNamed::F12), Some(NamedKey::F(12)));
        assert_eq!(named_key(&WNamed::Shift), None, "a modifier alone");
    }

    #[test]
    fn ctrl_is_the_shortcut_key_off_the_mac() {
        let m = modifiers(ModifiersState::CONTROL | ModifiersState::SHIFT);
        if cfg!(target_os = "macos") {
            assert!(m.ctrl && !m.cmd && m.shift);
            assert!(modifiers(ModifiersState::SUPER).cmd);
        } else {
            assert!(m.cmd && !m.ctrl && m.shift, "Ctrl-C copies");
            assert_eq!(modifiers(ModifiersState::SUPER), Modifiers::NONE, "the Windows key is the system's");
        }
    }

    #[test]
    fn characters_are_named_unmodified_and_lower_case() {
        let e = key_event(&ch("A"), Some(&ch("a")), Some("A"), true, false, Modifiers::SHIFT).unwrap();
        assert_eq!((e.key, e.text.as_deref()), (Key::Character('a'), Some("A")));
        let z = key_event(&ch("\u{1a}"), Some(&ch("z")), Some("\u{1a}"), true, false, Modifiers::CMD).unwrap();
        assert_eq!((z.key, z.text), (Key::Character('z'), None), "a shortcut types nothing");
        let up = key_event(&ch("a"), None, Some("a"), false, false, Modifiers::NONE).unwrap();
        assert!(!up.pressed && up.text.is_none(), "releases type nothing");
        let enter = key_event(&WKey::Named(WNamed::Enter), None, Some("\r"), true, true, Modifiers::NONE).unwrap();
        assert_eq!((enter.key, enter.text, enter.repeat), (Key::Named(NamedKey::Enter), None, true));
        assert!(key_event(&WKey::Named(WNamed::Shift), None, None, true, false, Modifiers::SHIFT).is_none());
    }

    #[test]
    fn alt_gr_still_types_off_the_mac() {
        let alt_gr = Modifiers { alt: true, cmd: true, ..Modifiers::NONE };
        let typed = typed_text("@", alt_gr);
        if cfg!(target_os = "macos") {
            assert_eq!(typed, None, "Cmd-Option is a shortcut on a Mac");
        } else {
            assert_eq!(typed.as_deref(), Some("@"));
        }
        assert_eq!(typed_text("é", Modifiers::NONE).as_deref(), Some("é"));
        assert_eq!(typed_text("\u{1b}", Modifiers::NONE), None);
    }

    #[test]
    fn wheels_scroll_by_lines_and_trackpads_by_points() {
        assert_eq!(scroll(MouseScrollDelta::LineDelta(0.0, -1.0), TouchPhase::Moved, 2.0), (vec2(0.0, -LINE), false, ScrollPhase::None));
        let (d, precise, phase) = scroll(MouseScrollDelta::PixelDelta(PhysicalPosition::new(4.0, -10.0)), TouchPhase::Started, 2.0);
        assert_eq!((d, precise, phase), (vec2(2.0, -5.0), true, ScrollPhase::Began));
    }

    #[test]
    fn composition_cancels_only_while_composing() {
        let mut composing = false;
        assert_eq!(ime(Ime::Preedit(String::new(), None), &mut composing), None);
        let pre = ime(Ime::Preedit("にほ".into(), Some((6, 6))), &mut composing);
        assert_eq!(pre, Some(ImeEvent::Preedit { text: "にほ".into(), cursor: Some((6, 6)) }));
        assert_eq!(ime(Ime::Preedit(String::new(), None), &mut composing), Some(ImeEvent::Cancel));
        assert_eq!(ime(Ime::Commit("日本".into()), &mut composing), Some(ImeEvent::Commit("日本".into())));
        assert_eq!(ime(Ime::Disabled, &mut composing), None);
    }

    #[test]
    fn presses_count_into_double_and_triple_clicks() {
        let mut clicks = Clicks::default();
        let t = Instant::now();
        let p = vui_core::pt(10.0, 10.0);
        assert_eq!(clicks.press(t, p, PointerButton::Primary), 1);
        assert_eq!(clicks.press(t + Duration::from_millis(200), vui_core::pt(12.0, 11.0), PointerButton::Primary), 2);
        assert_eq!(clicks.press(t + Duration::from_millis(400), p, PointerButton::Primary), 3);
        assert_eq!(clicks.press(t + Duration::from_millis(2000), p, PointerButton::Primary), 1, "too late");
        assert_eq!(clicks.press(t + Duration::from_millis(2100), vui_core::pt(40.0, 10.0), PointerButton::Primary), 1, "too far");
        assert_eq!(clicks.press(t + Duration::from_millis(2200), vui_core::pt(40.0, 10.0), PointerButton::Secondary), 1, "another button");
    }

    #[test]
    fn cursors() {
        assert_eq!(cursor(CursorIcon::Text), winit::window::CursorIcon::Text);
        assert_eq!(cursor(CursorIcon::ResizeHorizontal), winit::window::CursorIcon::EwResize);
    }
}
