//! Key bindings an app reads from a file: chords such as `cmd+shift+k`, sequences such as `g o`,
//! scopes that override the global map while a panel has focus, and a check that refuses a binding
//! which clashes with another one or with a key the host keeps for itself.
//!
//! ```
//! use vui_runtime::keymap::{Keymap, Resolve};
//! use vui_core::{Key, KeyEvent, Modifiers};
//!
//! let mut keys = Keymap::new();
//! keys.reserve("cmd+,", "Settings").unwrap();
//! keys.bind(Keymap::<&str>::GLOBAL, "cmd+k", "palette").unwrap();
//! assert!(keys.bind(Keymap::<&str>::GLOBAL, "cmd+,", "prefs").is_err());
//! let k = KeyEvent::press(Key::Character('k'), Modifiers::CMD);
//! assert_eq!(keys.press(&k, &[]), Resolve::Action(&"palette"));
//! ```

use std::fmt;

use vui_core::{Key, KeyEvent, Modifiers, NamedKey};

/// One key with its modifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: Key,
    pub mods: Modifiers,
}

impl Chord {
    pub fn of(e: &KeyEvent) -> Chord {
        let key = match &e.key {
            Key::Character(c) => Key::Character(c.to_ascii_lowercase()),
            k => k.clone(),
        };
        Chord { key, mods: e.mods }
    }

    /// `cmd+shift+k`, `ctrl+b`, `escape`, `f5`, `alt+arrowup`. Case doesn't matter; `⌘` `⇧` `⌃` `⌥`
    /// may stand for the modifiers.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let text = text.trim();
        let mut mods = Modifiers::NONE;
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let Some((last, held)) = parts.split_last() else { return Err(format!("\"{text}\": no key")) };
        for m in held {
            match m.to_ascii_lowercase().as_str() {
                "cmd" | "command" | "meta" | "super" | "⌘" => mods.cmd = true,
                "shift" | "⇧" => mods.shift = true,
                "ctrl" | "control" | "⌃" => mods.ctrl = true,
                "alt" | "opt" | "option" | "⌥" => mods.alt = true,
                other => return Err(format!("\"{text}\": \"{other}\" isn't a modifier")),
            }
        }
        let name = last.to_ascii_lowercase();
        let key = match name.as_str() {
            "" => return Err(format!("\"{text}\": no key")),
            "enter" | "return" => Key::Named(NamedKey::Enter),
            "tab" => Key::Named(NamedKey::Tab),
            "escape" | "esc" => Key::Named(NamedKey::Escape),
            "backspace" => Key::Named(NamedKey::Backspace),
            "delete" | "del" => Key::Named(NamedKey::Delete),
            "space" => Key::Named(NamedKey::Space),
            "left" | "arrowleft" => Key::Named(NamedKey::ArrowLeft),
            "right" | "arrowright" => Key::Named(NamedKey::ArrowRight),
            "up" | "arrowup" => Key::Named(NamedKey::ArrowUp),
            "down" | "arrowdown" => Key::Named(NamedKey::ArrowDown),
            "home" => Key::Named(NamedKey::Home),
            "end" => Key::Named(NamedKey::End),
            "pageup" => Key::Named(NamedKey::PageUp),
            "pagedown" => Key::Named(NamedKey::PageDown),
            f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => Key::Named(NamedKey::F(f[1..].parse().unwrap_or(1))),
            c if c.chars().count() == 1 => Key::Character(c.chars().next().unwrap_or(' ')),
            other => return Err(format!("\"{text}\": \"{other}\" isn't a key")),
        };
        Ok(Chord { key, mods })
    }
}

impl fmt::Display for Chord {
    /// As macOS menus write it: `⌃⌥⇧⌘K`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.mods;
        for (on, sign) in [(m.ctrl, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.cmd, "⌘")] {
            if on {
                f.write_str(sign)?;
            }
        }
        match self.key {
            Key::Character(c) => write!(f, "{}", c.to_ascii_uppercase()),
            Key::Named(n) => f.write_str(&match n {
                NamedKey::Enter => "↩".into(),
                NamedKey::Tab => "⇥".into(),
                NamedKey::Escape => "⎋".into(),
                NamedKey::Backspace => "⌫".into(),
                NamedKey::Delete => "⌦".into(),
                NamedKey::Space => "Space".into(),
                NamedKey::ArrowLeft => "←".into(),
                NamedKey::ArrowRight => "→".into(),
                NamedKey::ArrowUp => "↑".into(),
                NamedKey::ArrowDown => "↓".into(),
                NamedKey::Home => "↖".into(),
                NamedKey::End => "↘".into(),
                NamedKey::PageUp => "⇞".into(),
                NamedKey::PageDown => "⇟".into(),
                NamedKey::F(n) => format!("F{n}"),
            }),
        }
    }
}

/// One or more chords pressed one after another: `g o`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Keys(pub Vec<Chord>);

impl Keys {
    pub fn parse(text: &str) -> Result<Keys, String> {
        let chords = text.split_whitespace().map(Chord::parse).collect::<Result<Vec<_>, _>>()?;
        if chords.is_empty() { Err("no keys".into()) } else { Ok(Keys(chords)) }
    }

    /// True when one sequence starts the other, so both can't be told apart.
    fn overlaps(&self, other: &Keys) -> bool {
        let n = self.0.len().min(other.0.len());
        self.0[..n] == other.0[..n]
    }
}

impl fmt::Display for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// Why a binding was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum Clash {
    /// The text doesn't parse.
    Parse(String),
    /// The host keeps these keys, for what's named.
    Host { keys: Keys, owner: String },
    /// Another binding in the same scope has these keys or starts them.
    Taken { keys: Keys, scope: String, by: String },
}

impl fmt::Display for Clash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clash::Parse(e) => f.write_str(e),
            Clash::Host { keys, owner } => write!(f, "{keys} is kept for {owner}"),
            Clash::Taken { keys, scope, by } => write!(f, "{keys} is already {by} in {scope}"),
        }
    }
}

impl std::error::Error for Clash {}

/// What a key press came to.
#[derive(Debug, PartialEq)]
pub enum Resolve<'a, A> {
    Action(&'a A),
    /// The start of a sequence: wait for the next key.
    Pending,
    /// No binding; the key goes on to the focused widget.
    None,
}

struct Binding<A> {
    scope: String,
    keys: Keys,
    action: A,
}

/// The app's bindings, by scope, and the host's keys none of them may take.
pub struct Keymap<A> {
    bindings: Vec<Binding<A>>,
    reserved: Vec<(Keys, String)>,
    pending: Vec<Chord>,
}

impl<A> Default for Keymap<A> {
    fn default() -> Self {
        Keymap { bindings: Vec::new(), reserved: Vec::new(), pending: Vec::new() }
    }
}

impl<A: fmt::Debug> Keymap<A> {
    /// The scope that's always on.
    pub const GLOBAL: &'static str = "global";

    pub fn new() -> Keymap<A> {
        Keymap::default()
    }

    /// Keeps `keys` for the host, `owner` naming what they do there ("Settings", "the tabs").
    pub fn reserve(&mut self, keys: &str, owner: &str) -> Result<(), Clash> {
        let keys = Keys::parse(keys).map_err(Clash::Parse)?;
        self.reserved.push((keys, owner.to_string()));
        Ok(())
    }

    /// Binds `keys` to `action` while `scope` is on. A scope may bind keys the global map has (it
    /// wins while on); it may not bind keys it already has, a sequence one of its own starts, or a
    /// key the host keeps.
    pub fn bind(&mut self, scope: &str, keys: &str, action: A) -> Result<(), Clash> {
        let keys = Keys::parse(keys).map_err(Clash::Parse)?;
        if let Some((_, owner)) = self.reserved.iter().find(|(k, _)| k.overlaps(&keys)) {
            return Err(Clash::Host { keys, owner: owner.clone() });
        }
        if let Some(b) = self.bindings.iter().find(|b| b.scope == scope && b.keys.overlaps(&keys)) {
            return Err(Clash::Taken { keys, scope: scope.to_string(), by: format!("{:?}", b.action) });
        }
        self.bindings.push(Binding { scope: scope.to_string(), keys, action });
        Ok(())
    }

    /// Drops every binding of `scope`, so a file can be read again.
    pub fn clear(&mut self, scope: &str) {
        self.bindings.retain(|b| b.scope != scope);
    }

    /// The keys bound to `action`, for a menu or the palette to show.
    pub fn keys_for(&self, action: &A) -> Option<&Keys>
    where
        A: PartialEq,
    {
        self.bindings.iter().find(|b| b.action == *action).map(|b| &b.keys)
    }

    /// Every binding, as (scope, keys, action).
    pub fn bindings(&self) -> impl Iterator<Item = (&str, &Keys, &A)> {
        self.bindings.iter().map(|b| (b.scope.as_str(), &b.keys, &b.action))
    }

    /// True while a sequence has begun.
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Forgets a sequence half typed, after a pause or a click.
    pub fn cancel(&mut self) {
        self.pending.clear();
    }

    /// Feeds a key down. `scopes` are those on now, the most specific first; the global map comes
    /// after them. Key ups, repeats inside a sequence and bare modifiers are ignored.
    pub fn press(&mut self, e: &KeyEvent, scopes: &[&str]) -> Resolve<'_, A> {
        if !e.pressed {
            return Resolve::None;
        }
        let mut typed = std::mem::take(&mut self.pending);
        typed.push(Chord::of(e));
        let mut started = false;
        let mut hit = None;
        'scopes: for scope in scopes.iter().copied().chain(std::iter::once(Self::GLOBAL)) {
            for (i, b) in self.bindings.iter().enumerate().filter(|(_, b)| b.scope == scope) {
                if b.keys.0 == typed {
                    hit = Some(i);
                    break 'scopes;
                }
                started |= b.keys.0.len() > typed.len() && b.keys.0[..typed.len()] == typed[..];
            }
        }
        if let Some(i) = hit {
            return Resolve::Action(&self.bindings[i].action);
        }
        if started {
            self.pending = typed;
            return Resolve::Pending;
        }
        // A sequence that went nowhere: try the last key alone.
        if typed.len() > 1 {
            return self.press(e, scopes);
        }
        Resolve::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char, mods: Modifiers) -> KeyEvent {
        KeyEvent::press(Key::Character(c), mods)
    }

    #[test]
    fn chords_parse_and_print_as_macos_menus_do() {
        let c = Chord::parse("cmd+shift+k").unwrap();
        assert_eq!(c, Chord { key: Key::Character('k'), mods: Modifiers { cmd: true, shift: true, ..Modifiers::NONE } });
        assert_eq!(c.to_string(), "⇧⌘K");
        assert_eq!(Chord::parse("⌃b").unwrap_err(), "\"⌃b\": \"⌃b\" isn't a key");
        assert_eq!(Chord::parse("⌃+b").unwrap().to_string(), "⌃B");
        assert_eq!(Chord::parse("F5").unwrap().key, Key::Named(NamedKey::F(5)));
        assert_eq!(Keys::parse("g o").unwrap().to_string(), "G O");
        assert!(Chord::parse("hyper+k").is_err());
    }

    #[test]
    fn a_host_key_or_a_taken_one_is_refused() {
        let mut k = Keymap::new();
        k.reserve("cmd+1", "the tabs").unwrap();
        assert_eq!(k.bind("global", "cmd+1", "watch").unwrap_err().to_string(), "⌘1 is kept for the tabs");
        k.bind("global", "g o", "orders").unwrap();
        assert!(matches!(k.bind("global", "g", "go"), Err(Clash::Taken { .. })), "a key that starts a sequence");
        assert!(matches!(k.bind("global", "g o", "other"), Err(Clash::Taken { .. })));
        k.bind("chart", "g", "grid").unwrap();
    }

    #[test]
    fn sequences_wait_and_scopes_win_over_the_global_map() {
        let mut k = Keymap::new();
        k.bind("global", "g o", "orders").unwrap();
        k.bind("global", "escape", "close").unwrap();
        k.bind("ticket", "escape", "clear").unwrap();
        assert_eq!(k.press(&key('g', Modifiers::NONE), &[]), Resolve::Pending);
        assert_eq!(k.press(&key('o', Modifiers::NONE), &[]), Resolve::Action(&"orders"));
        let esc = KeyEvent::press(Key::Named(NamedKey::Escape), Modifiers::NONE);
        assert_eq!(k.press(&esc, &["ticket"]), Resolve::Action(&"clear"));
        assert_eq!(k.press(&esc, &[]), Resolve::Action(&"close"));
        // A sequence broken off: the last key counts by itself.
        assert_eq!(k.press(&key('g', Modifiers::NONE), &[]), Resolve::Pending);
        assert_eq!(k.press(&esc, &[]), Resolve::Action(&"close"));
        assert!(!k.pending());
        assert_eq!(k.keys_for(&"orders").map(|s| s.to_string()).as_deref(), Some("G O"));
    }
}
