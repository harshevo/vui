//! A segmented control: a few choices side by side, exactly one of them chosen.

use std::rc::Rc;

use vui_core::{Fill, Spring, TextRole, Tone};
use vui_runtime::{Builder, Div, Focus, FocusGroup, Parent, Radius, Role, Styled, row};

use crate::item::{Height, Item, Place, Spec};

/// How tall a segment is: the track is 2 points bigger on every side.
const SEGMENT_HEIGHT: f32 = 28.0;
const TRACK_PAD: f32 = 2.0;

/// The choices as segments on a grey track. The chosen one is lifted out in the field colour. The
/// control is one Tab stop; the arrow keys move between segments and choose as they go.
pub fn segmented<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    build(options.into_iter().map(|(value, label)| (value, label, None)), selected, on_select)
}

/// Like [`segmented`], with the track keyed `key` and each segment `{key}-{id}`, so tests and
/// assistive tools can find one choice: `(value, id, label)`.
pub fn segmented_keyed<M: 'static, T: PartialEq + Clone + 'static>(
    key: &str,
    options: impl IntoIterator<Item = (T, String, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    build(
        options.into_iter().map(|(value, id, label)| (value, label, Some(format!("{key}-{id}")))),
        selected,
        on_select,
    )
    .key(key.to_string())
}

fn build<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String, Option<String>)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    let on_select = Rc::new(on_select);
    let mut track = row::<M>()
        .padding(TRACK_PAD)
        .gap(TRACK_PAD)
        .bg(Tone::Wash)
        .rounded(Radius::Control)
        .focus_group(FocusGroup::horizontal())
        .role(Role::Group)
        .align_self(vui_runtime::Align::Start);
    for (value, label, key) in options {
        let chosen = value == *selected;
        let mut spec = Spec::new(label, Role::RadioButton);
        spec.weight = 500;
        spec.text = TextRole::Body;
        spec.height = Height::Content;
        spec.place = Place::Center;
        spec.label_color = if chosen { Tone::Accent } else { Tone::Text2 }.into();
        spec.toggled = Some(chosen);
        spec.focus = Focus::Keyboard;
        let pick = {
            let on_select = on_select.clone();
            let value = value.clone();
            move || on_select(value.clone())
        };
        let segment = Item::from_spec(spec)
            .height(SEGMENT_HEIGHT)
            .padding_x(12.0)
            .rounded(Radius::Px(6.0))
            .bg(if chosen { Fill::Tone(Tone::Accent, 0.14) } else { Fill::NONE })
            .hover_bg(if chosen { Fill::NONE } else { Fill::from(Tone::Hover) })
            .transition(Spring::SNAPPY)
            .tab_default(chosen)
            .select_on_arrow(pick.clone())
            .on_click_with(pick);
        let segment = match key {
            Some(key) => segment.key(key),
            None => segment,
        };
        track = track.child(segment);
    }
    track
}
