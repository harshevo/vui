//! What an element does with input: its handlers, and mapping them to a parent's messages.

use super::*;

pub(crate) type Handler<M> = Box<dyn Fn(&Click) -> M>;
pub(crate) type KeyHandler<M> = Box<dyn Fn(&KeyEvent) -> Option<M>>;

/// What an element does with input. Handlers build the message; the runtime delivers it.
pub(crate) struct Input<M> {
    pub click: Option<Handler<M>>,
    pub press: Option<Handler<M>>,
    pub double_click: Option<Handler<M>>,
    pub secondary: Option<Handler<M>>,
    pub hover: Option<Box<dyn Fn(bool) -> M>>,
    pub key: Option<KeyHandler<M>>,
    pub drop: Option<Box<dyn Fn(Vec<PathBuf>) -> M>>,
    pub focus: Option<Focus>,
    pub group: Option<FocusGroup>,
    pub tab_default: bool,
    pub cursor: Option<CursorIcon>,
    pub disabled: bool,
    pub pointer_events: bool,
    pub window_drag: bool,
    /// Sent when a press lands outside the element, or Escape goes unanswered while it shows.
    pub dismiss: Option<Box<dyn Fn() -> M>>,
    /// Takes focus when it first appears.
    pub autofocus: bool,
    /// Shown under the element after the pointer rests on it.
    pub tooltip: Option<String>,
    /// Keeps Tab inside the element while it shows: a dialog.
    pub trap_focus: bool,
}

impl<M> Default for Input<M> {
    fn default() -> Input<M> {
        Input {
            click: None,
            press: None,
            double_click: None,
            secondary: None,
            hover: None,
            key: None,
            drop: None,
            focus: None,
            group: None,
            tab_default: false,
            cursor: None,
            disabled: false,
            pointer_events: true,
            window_drag: false,
            dismiss: None,
            autofocus: false,
            tooltip: None,
            trap_focus: false,
        }
    }
}

impl<M: 'static> Input<M> {
    pub(crate) fn map<N: 'static>(self, f: &Rc<dyn Fn(M) -> N>) -> Input<N> {
        let click = |h: Option<Handler<M>>| -> Option<Handler<N>> {
            h.map(|h| {
                let f = f.clone();
                Box::new(move |c: &Click| f(h(c))) as Handler<N>
            })
        };
        Input {
            click: click(self.click),
            press: click(self.press),
            double_click: click(self.double_click),
            secondary: click(self.secondary),
            hover: self.hover.map(|h| {
                let f = f.clone();
                Box::new(move |on: bool| f(h(on))) as Box<dyn Fn(bool) -> N>
            }),
            key: self.key.map(|h| {
                let f = f.clone();
                Box::new(move |k: &KeyEvent| h(k).map(|m| f(m))) as KeyHandler<N>
            }),
            drop: self.drop.map(|h| {
                let f = f.clone();
                Box::new(move |paths: Vec<PathBuf>| f(h(paths))) as Box<dyn Fn(Vec<PathBuf>) -> N>
            }),
            dismiss: self.dismiss.map(|h| {
                let f = f.clone();
                Box::new(move || f(h())) as Box<dyn Fn() -> N>
            }),
            focus: self.focus,
            group: self.group,
            tab_default: self.tab_default,
            cursor: self.cursor,
            disabled: self.disabled,
            pointer_events: self.pointer_events,
            window_drag: self.window_drag,
            autofocus: self.autofocus,
            tooltip: self.tooltip,
            trap_focus: self.trap_focus,
        }
    }
}

impl<M> Input<M> {
    /// Pressing it means something, so the runtime tracks the press.
    pub fn clickable(&self) -> bool {
        self.click.is_some() || self.press.is_some() || self.double_click.is_some()
    }
}
