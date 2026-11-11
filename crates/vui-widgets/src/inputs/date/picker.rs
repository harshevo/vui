//! The date picker: a field showing the date, which opens a calendar under itself.

use std::rc::Rc;

use vui_core::{Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Focus, IntoElement, Parent, Radius, Role, Styled, label, row};

use super::calendar::{calendar, card};
use super::Date;
use crate::display::icon::icon;
use crate::overlay::popover;

/// What a [`date_picker`] asks for.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DateEvent {
    Open(bool),
    /// Show this month in the calendar.
    Month(Date),
    /// This day was picked; close the calendar.
    Pick(Date),
}

/// A field showing `value`, or a placeholder, which opens a calendar. Keep the open flag and the
/// month on show in a [`DatePicker`] and hand it what `on` sends.
///
/// ```
/// use vui_widgets::{Date, DateEvent, DatePicker, date_picker};
///
/// #[derive(Clone)]
/// enum Msg { Due(DateEvent) }
///
/// let mut state = DatePicker::default();
/// let mut due = None;
/// if let Some(day) = state.apply(DateEvent::Pick(Date::new(2026, 12, 1).unwrap())) {
///     due = Some(day);
/// }
/// let field = date_picker(due, &state, Msg::Due);
/// ```
pub fn date_picker<M: Clone + 'static>(value: Option<Date>, state: &DatePicker, on: impl Fn(DateEvent) -> M + 'static) -> impl IntoElement<M> {
    let on: Rc<dyn Fn(DateEvent) -> M> = Rc::new(on);
    let text = value.map_or_else(|| "Pick a date".to_string(), Date::short);
    let field = row::<M>()
        .height(32.0)
        .padding_x(10.0)
        .gap(8.0)
        .min_width(160.0)
        .align_items(Align::Center)
        .bg(Tone::Field)
        .border(1.0, Tone::Line)
        .rounded(Radius::Control)
        .hover_bg(Tone::Hover)
        .transition(Spring::SNAPPY)
        .focus_mode(Focus::Keyboard)
        .role(Role::Button)
        .name(text.clone())
        .cursor(vui_core::CursorIcon::Pointer)
        .on_click(on(DateEvent::Open(!state.open)))
        .children((icon(set::CALENDAR).color(Tone::Text2), label(text).grow(1.0).color(if value.is_some() { Tone::Text } else { Tone::Text3 })));
    let shown = state.month.or(value).unwrap_or_else(Date::today);
    let (pick, month) = (on.clone(), on.clone());
    let cal = calendar(shown).selected(value).today(Date::today()).on_pick(move |d| pick(DateEvent::Pick(d))).on_month(move |d| month(DateEvent::Month(d)));
    popover(field, state.open, on(DateEvent::Open(false)), card().child(cal))
}

/// The state of a [`date_picker`]: whether the calendar shows, and which month it shows.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DatePicker {
    pub open: bool,
    /// `None` shows the value's month, or this month.
    pub month: Option<Date>,
}

impl DatePicker {
    /// Updates the state, and returns the day picked, if one was.
    pub fn apply(&mut self, event: DateEvent) -> Option<Date> {
        match event {
            DateEvent::Open(open) => {
                self.open = open;
                if !open {
                    self.month = None;
                }
                None
            }
            DateEvent::Month(m) => {
                self.month = Some(m);
                None
            }
            DateEvent::Pick(d) => {
                *self = DatePicker::default();
                Some(d)
            }
        }
    }
}

