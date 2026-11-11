//! A combobox: a text field that suggests options as it is typed in, from a list filtered by fuzzy
//! matching. Up and Down move through the suggestions, Return picks one, Escape closes them.

use std::rc::Rc;

use vui_core::{Key, KeyEvent, NamedKey, Tone};
use vui_runtime::{Builder, Element, IntoElement, Parent, Role, Styled, div, small};

use super::picker::{Picker, PickerEvent, fuzzy_filter, step};
use super::text_field::text_field;
use crate::overlay::{Popover, menu, menu_item, popover};

/// The most suggestions shown at once.
const SHOWN: usize = 50;

/// Keys that steer a filtered list from a field that keeps the focus: Up, Down and Return.
pub(crate) fn steer<M: 'static>(
    on: Rc<dyn Fn(PickerEvent) -> M>,
    open: bool,
    active: usize,
    matches: Vec<(usize, String)>,
) -> impl Fn(&KeyEvent) -> Option<M> + 'static {
    move |k: &KeyEvent| {
        if !k.pressed || k.mods.cmd || k.mods.ctrl || k.mods.alt {
            return None;
        }
        match k.key {
            Key::Named(NamedKey::ArrowDown) if !open => Some(on(PickerEvent::Open(true))),
            Key::Named(NamedKey::ArrowDown) => Some(on(PickerEvent::Highlight(step(active, matches.len(), true)))),
            Key::Named(NamedKey::ArrowUp) if open => Some(on(PickerEvent::Highlight(step(active, matches.len(), false)))),
            Key::Named(NamedKey::Enter) if open => {
                matches.get(active.min(matches.len().saturating_sub(1))).map(|(i, label)| on(PickerEvent::Pick(*i, label.clone())))
            }
            _ => None,
        }
    }
}

/// A field with suggestions from `options`, filtered by what has been typed. The app keeps a
/// [`Picker`] and hands it what `on` sends; [`Picker::apply`] returns the index of an option when one
/// is chosen, and leaves its label in the field.
///
/// ```
/// use vui_widgets::{Picker, PickerEvent, combobox};
///
/// #[derive(Clone)]
/// enum Msg { City(PickerEvent) }
///
/// let cities = ["Lisbon", "London", "Los Angeles", "Lyon"].map(String::from);
/// let state = Picker::new();
/// let field = combobox(&state, &cities, Msg::City).placeholder("City");
/// ```
pub fn combobox<M: Clone + 'static>(state: &Picker, options: &[String], on: impl Fn(PickerEvent) -> M + 'static) -> Combobox<M> {
    let on: Rc<dyn Fn(PickerEvent) -> M> = Rc::new(on);
    let matches: Vec<(usize, String)> =
        fuzzy_filter(&state.query, options.iter().map(String::as_str)).into_iter().take(SHOWN).map(|i| (i, options[i].clone())).collect();
    Combobox { state: state.clone(), matches, on, placeholder: String::new(), empty: "No matches".into() }
}

/// See [`combobox`].
pub struct Combobox<M: 'static> {
    state: Picker,
    matches: Vec<(usize, String)>,
    on: Rc<dyn Fn(PickerEvent) -> M>,
    placeholder: String,
    empty: String,
}

impl<M: Clone + 'static> Combobox<M> {
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// What the list says when nothing matches; "No matches" by default.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty = text.into();
        self
    }

    fn popover(self) -> Popover<M> {
        let Combobox { state, matches, on, placeholder, empty } = self;
        let active = state.active.min(matches.len().saturating_sub(1));
        let typed = on.clone();
        let field = div::<M>().on_key(steer(on.clone(), state.open, active, matches.clone())).child(
            text_field(state.query.clone()).pass_keys().placeholder(placeholder).role(Role::SearchInput).on_change(move |q| typed(PickerEvent::Query(q))),
        );
        let items: Vec<Element<M>> = if matches.is_empty() {
            vec![small(empty).color(Tone::Text3).padding(8.0).into_element()]
        } else {
            matches
                .into_iter()
                .enumerate()
                .map(|(n, (i, label))| {
                    let pick = on(PickerEvent::Pick(i, label.clone()));
                    let item = menu_item(label).on_click(pick);
                    if n == active { item.bg(Tone::Hover).into_element() } else { item.into_element() }
                })
                .collect()
        };
        popover(field, state.open, on(PickerEvent::Open(false)), menu(items).max_height(280.0).overflow(vui_runtime::Overflow::Scroll))
            .match_width()
            .keep_focus()
    }
}

impl<M: Clone + 'static> IntoElement<M> for Combobox<M> {
    fn into_element(self) -> Element<M> {
        self.popover().into_element()
    }
}

impl<M: Clone + 'static> vui_runtime::IntoChildren<M> for Combobox<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}
