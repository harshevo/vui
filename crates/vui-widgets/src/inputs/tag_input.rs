//! A tag input: chips for the tags so far, and a field for the next one. Return or a comma adds the
//! tag, Backspace in the empty field removes the last one.

use std::rc::Rc;

use vui_core::{Key, KeyEvent, NamedKey, Tone};
use vui_runtime::{Align, Builder, Div, Parent, Radius, Styled, row};

use super::text_field::text_field;
use crate::display::chip;

/// What a [`tag_input`] asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum TagEvent {
    /// The text being typed changed.
    Draft(String),
    /// Add this tag, and empty the draft.
    Add(String),
    /// Remove the tag at this index.
    Remove(usize),
}

/// `tags` as removable chips, then a field holding `draft`. Hand what `on` sends to the app's tag
/// list; an `Add` is already trimmed and never empty.
///
/// ```
/// use vui_widgets::{TagEvent, tag_input};
///
/// #[derive(Clone)]
/// enum Msg { Tags(TagEvent) }
///
/// let tags = vec!["rust".to_string(), "ui".to_string()];
/// let field = tag_input(&tags, "", Msg::Tags);
/// ```
pub fn tag_input<M: Clone + 'static>(tags: &[String], draft: &str, on: impl Fn(TagEvent) -> M + 'static) -> Div<M> {
    let on: Rc<dyn Fn(TagEvent) -> M> = Rc::new(on);
    let typed = on.clone();
    let field = text_field(draft.to_string())
        .key("vui-tag-draft")
        .name("Add a tag")
        .pass_keys()
        .bare()
        .basis(80.0)
        .min_width(80.0)
        .grow(1.0)
        .placeholder(if tags.is_empty() { "Add a tag" } else { "" })
        .on_change(move |text| match text.strip_suffix(',') {
            Some(tag) if !tag.trim().is_empty() => typed(TagEvent::Add(tag.trim().to_string())),
            Some(_) => typed(TagEvent::Draft(String::new())),
            None => typed(TagEvent::Draft(text)),
        });
    let keys = {
        let (on, draft, count) = (on.clone(), draft.trim().to_string(), tags.len());
        move |k: &KeyEvent| match k.key {
            Key::Named(NamedKey::Enter) if k.pressed && !draft.is_empty() => Some(on(TagEvent::Add(draft.clone()))),
            Key::Named(NamedKey::Backspace) if k.pressed && count > 0 => Some(on(TagEvent::Remove(count - 1))),
            _ => None,
        }
    };
    let chips: Vec<_> = tags.iter().enumerate().map(|(i, t)| chip(t.clone(), Some(on(TagEvent::Remove(i))))).collect();
    row()
        .wrap()
        .gap(6.0)
        .padding(4.0)
        .padding_left(6.0)
        .min_height(36.0)
        .align_items(Align::Center)
        .bg(Tone::Field)
        .border(1.0, Tone::Line)
        .rounded(Radius::Control)
        .on_key(keys)
        .children(chips)
        .child(field)
}
