//! Asking the system for things beyond the window: file dialogs, notifications, full screen, the
//! app's menus and keyboard shortcuts, and an item in the menu bar.
//!
//! Requests go out as [`Effect`](crate::Effect)s that the backend carries out. Answers come back
//! through [`Ui::answer`](crate::Ui::answer) and [`Ui::choose_menu`](crate::Ui::choose_menu), and
//! reach the app as its own messages, so the app never touches platform types and tests can play
//! the system's part.

use std::path::PathBuf;
use std::rc::Rc;

use vui_core::Modifiers;

/// An open or save dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileDialog {
    pub title: Option<String>,
    /// The folder it starts in.
    pub directory: Option<PathBuf>,
    /// For saving: the name it suggests.
    pub file_name: Option<String>,
    /// File extensions that can be picked or saved, without dots; empty for any.
    pub extensions: Vec<String>,
    /// For opening: several files may be picked.
    pub multiple: bool,
    /// For opening: folders are picked instead of files.
    pub folders: bool,
}

impl FileDialog {
    pub fn new() -> FileDialog {
        FileDialog::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> FileDialog {
        self.title = Some(title.into());
        self
    }

    pub fn directory(mut self, dir: impl Into<PathBuf>) -> FileDialog {
        self.directory = Some(dir.into());
        self
    }

    pub fn file_name(mut self, name: impl Into<String>) -> FileDialog {
        self.file_name = Some(name.into());
        self
    }

    pub fn extensions<S: Into<String>>(mut self, extensions: impl IntoIterator<Item = S>) -> FileDialog {
        self.extensions = extensions.into_iter().map(Into::into).collect();
        self
    }

    pub fn multiple(mut self) -> FileDialog {
        self.multiple = true;
        self
    }

    pub fn folders(mut self) -> FileDialog {
        self.folders = true;
        self
    }
}

/// A notification shown by the system, outside the window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub sound: bool,
}

impl Notification {
    pub fn new(title: impl Into<String>) -> Notification {
        Notification { title: title.into(), ..Notification::default() }
    }

    pub fn body(mut self, body: impl Into<String>) -> Notification {
        self.body = body.into();
        self
    }

    pub fn sound(mut self) -> Notification {
        self.sound = true;
        self
    }
}

/// A keyboard shortcut for a menu item: a character and the keys held with it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: char,
    pub mods: Modifiers,
}

impl Shortcut {
    pub const fn new(key: char, mods: Modifiers) -> Shortcut {
        Shortcut { key, mods }
    }

    /// Command and `key`: the usual shortcut.
    pub const fn cmd(key: char) -> Shortcut {
        Shortcut { key, mods: Modifiers::CMD }
    }

    /// Command, Shift and `key`.
    pub const fn cmd_shift(key: char) -> Shortcut {
        Shortcut { key, mods: Modifiers { shift: true, cmd: true, ctrl: false, alt: false } }
    }
}

/// What a menu item sends when chosen.
pub type MenuAction<M> = Rc<dyn Fn() -> M>;

/// One entry in a menu.
pub enum MenuItem<M> {
    Item { title: String, shortcut: Option<Shortcut>, enabled: bool, checked: bool, action: MenuAction<M> },
    Separator,
}

impl<M: 'static> MenuItem<M> {
    /// An item that sends `message` when chosen.
    pub fn new(title: impl Into<String>, message: M) -> MenuItem<M>
    where
        M: Clone,
    {
        MenuItem::Item { title: title.into(), shortcut: None, enabled: true, checked: false, action: Rc::new(move || message.clone()) }
    }

    pub fn separator() -> MenuItem<M> {
        MenuItem::Separator
    }

    /// The shortcut that chooses it, shown beside it.
    pub fn shortcut(mut self, shortcut: Shortcut) -> MenuItem<M> {
        if let MenuItem::Item { shortcut: s, .. } = &mut self {
            *s = Some(shortcut);
        }
        self
    }

    pub fn enabled(mut self, on: bool) -> MenuItem<M> {
        if let MenuItem::Item { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }

    /// A tick beside it.
    pub fn checked(mut self, on: bool) -> MenuItem<M> {
        if let MenuItem::Item { checked, .. } = &mut self {
            *checked = on;
        }
        self
    }
}

/// A menu in the app's menu bar: one titled "File" goes after the app menu, the rest after Edit
/// and before Window.
pub struct Menu<M> {
    pub title: String,
    pub items: Vec<MenuItem<M>>,
}

impl<M> Menu<M> {
    pub fn new(title: impl Into<String>, items: impl IntoIterator<Item = MenuItem<M>>) -> Menu<M> {
        Menu { title: title.into(), items: items.into_iter().collect() }
    }
}

/// An item in the system's menu bar, at the right: a short title, and a menu when clicked.
pub struct StatusItem<M> {
    pub title: String,
    pub items: Vec<MenuItem<M>>,
}

impl<M> StatusItem<M> {
    pub fn new(title: impl Into<String>, items: impl IntoIterator<Item = MenuItem<M>>) -> StatusItem<M> {
        StatusItem { title: title.into(), items: items.into_iter().collect() }
    }
}

/// A menu as the backend builds it: titles, shortcuts and the ids to report back.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuSpec {
    pub title: String,
    pub items: Vec<ItemSpec>,
}

/// One entry of a [`MenuSpec`]. `id` is what the backend hands to [`Ui::choose_menu`](crate::Ui::choose_menu).
#[derive(Clone, Debug, PartialEq)]
pub enum ItemSpec {
    Item { id: u64, title: String, shortcut: Option<Shortcut>, enabled: bool, checked: bool },
    Separator,
}

/// Flattens `items` into specs, numbering the actions from `next` and collecting them.
pub(crate) fn specs<M>(items: Vec<MenuItem<M>>, next: &mut u64, actions: &mut Vec<(u64, MenuAction<M>)>) -> Vec<ItemSpec> {
    items
        .into_iter()
        .map(|item| match item {
            MenuItem::Separator => ItemSpec::Separator,
            MenuItem::Item { title, shortcut, enabled, checked, action } => {
                let id = *next;
                *next += 1;
                actions.push((id, action));
                ItemSpec::Item { id, title, shortcut, enabled, checked }
            }
        })
        .collect()
}
