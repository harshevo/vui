//! Identity for elements.

use std::sync::Arc;

/// Names an element. Give one to anything whose position among its siblings can change (a list that
/// reorders keeps each row's hover and scroll state this way) and to anything the app needs to find,
/// such as an anchor for a popover.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Num(u64),
    Name(Arc<str>),
}

impl From<&str> for Key {
    fn from(s: &str) -> Key {
        Key::Name(s.into())
    }
}

impl From<String> for Key {
    fn from(s: String) -> Key {
        Key::Name(s.into())
    }
}

impl From<&String> for Key {
    fn from(s: &String) -> Key {
        Key::Name(s.as_str().into())
    }
}

impl From<u64> for Key {
    fn from(n: u64) -> Key {
        Key::Num(n)
    }
}

impl From<usize> for Key {
    fn from(n: usize) -> Key {
        Key::Num(n as u64)
    }
}

impl From<u32> for Key {
    fn from(n: u32) -> Key {
        Key::Num(n as u64)
    }
}

impl From<i32> for Key {
    fn from(n: i32) -> Key {
        Key::Num(n as u64)
    }
}

/// A node's identity for as long as it lives. Unlike a `Key`, the runtime assigns it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_compare_by_value() {
        assert_eq!(Key::from("a"), Key::from(String::from("a")));
        assert_ne!(Key::from("a"), Key::from("b"));
        assert_eq!(Key::from(3usize), Key::from(3u64));
        assert_ne!(Key::from("3"), Key::from(3u64));
    }
}
