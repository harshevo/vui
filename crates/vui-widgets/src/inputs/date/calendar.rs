//! The month grid: a month's days under their weekdays, with arrows to the months either side.

use std::rc::Rc;

use vui_core::{Fill, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Element, FocusGroup, IntoElement, Justify, Parent, Radius, Role, Styled, column, div, grid, label, row, small};

use super::{Date, days_in_month};
use crate::buttons::button::icon_button;

const CELL: f32 = 34.0;

/// A month of days to pick from: `calendar(month).selected(day).on_pick(Msg::Day).on_month(Msg::Month)`.
/// `month` is any day of the month to show.
///
/// ```
/// use vui_widgets::{Date, calendar};
///
/// #[derive(Clone)]
/// enum Msg { Day(Date), Month(Date) }
/// let november = Date::new(2026, 11, 1).unwrap();
/// let cal = calendar(november).selected(Date::new(2026, 11, 11)).on_pick(Msg::Day).on_month(Msg::Month);
/// ```
pub fn calendar<M: 'static>(month: Date) -> Calendar<M> {
    Calendar { month: month.first_of_month(), selected: None, range: None, today: None, min: None, max: None, on_pick: None, on_month: None }
}

/// See [`calendar`].
pub struct Calendar<M: 'static> {
    month: Date,
    selected: Option<Date>,
    range: Option<(Date, Date)>,
    today: Option<Date>,
    min: Option<Date>,
    max: Option<Date>,
    on_pick: Option<Rc<dyn Fn(Date) -> M>>,
    on_month: Option<Rc<dyn Fn(Date) -> M>>,
}

impl<M: 'static> Calendar<M> {
    pub fn selected(mut self, day: Option<Date>) -> Self {
        self.selected = day;
        self
    }

    /// Shades the days from `start` to `end`, both included: a stay, a reporting period.
    pub fn range(mut self, start: Date, end: Date) -> Self {
        self.range = Some((start.min(end), start.max(end)));
        self
    }

    /// Rings this day; `Date::today()` usually.
    pub fn today(mut self, day: Date) -> Self {
        self.today = Some(day);
        self
    }

    /// Days before `day` cannot be picked.
    pub fn min(mut self, day: Date) -> Self {
        self.min = Some(day);
        self
    }

    /// Days after `day` cannot be picked.
    pub fn max(mut self, day: Date) -> Self {
        self.max = Some(day);
        self
    }

    pub fn on_pick(mut self, f: impl Fn(Date) -> M + 'static) -> Self {
        self.on_pick = Some(Rc::new(f));
        self
    }

    /// Sent with the first day of the month an arrow asks for. Without it there are no arrows.
    pub fn on_month(mut self, f: impl Fn(Date) -> M + 'static) -> Self {
        self.on_month = Some(Rc::new(f));
        self
    }
}

impl<M: Clone + 'static> IntoElement<M> for Calendar<M> {
    fn into_element(self) -> Element<M> {
        let Calendar { month, selected, range, today, min, max, on_pick, on_month } = self;
        let arrows = on_month.map(|f| {
            row().gap(2.0).children((
                icon_button(set::CHEVRON_LEFT, "Previous month").small().on_click(f(month.add_months(-1))),
                icon_button(set::CHEVRON_RIGHT, "Next month").small().on_click(f(month.add_months(1))),
            ))
        });
        let header = row()
            .height(32.0)
            .padding_left(6.0)
            .align_items(Align::Center)
            .justify(Justify::SpaceBetween)
            .children((label(format!("{} {}", month.month_name(), month.year)).weight(600), arrows));

        let mut cells: Vec<Element<M>> = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
            .iter()
            .map(|d| row().height(28.0).justify(Justify::Center).align_items(Align::Center).child(small(*d).color(Tone::Text3)).into_element())
            .collect();
        for _ in 0..month.weekday() {
            cells.push(div().into_element());
        }
        for day in 1..=days_in_month(month.year, month.month) {
            let date = Date { day, ..month };
            let picked = selected == Some(date) || range.is_some_and(|(a, b)| date == a || date == b);
            let inside = range.is_some_and(|(a, b)| date > a && date < b);
            let allowed = min.is_none_or(|m| date >= m) && max.is_none_or(|m| date <= m);
            let (fill, text) = if picked {
                (Fill::from(Tone::Accent), Tone::OnAccent)
            } else if inside {
                (Fill::Tone(Tone::Accent, 0.14), Tone::Text)
            } else {
                (Fill::NONE, if allowed { Tone::Text } else { Tone::Text3 })
            };
            let mut cell = row()
                .key(day as usize)
                .height(CELL)
                .justify(Justify::Center)
                .align_items(Align::Center)
                .rounded(Radius::Control)
                .bg(fill)
                .transition(Spring::SNAPPY)
                .role(Role::Button)
                .name(date.short())
                .child(label(day.to_string()).color(text).weight(if picked { 600 } else { 400 }));
            if today == Some(date) && !picked {
                cell = cell.border(1.0, Tone::Accent);
            }
            if allowed && let Some(f) = &on_pick {
                cell = cell.hover_bg(if picked { Fill::from(Tone::Accent) } else { Fill::from(Tone::Hover) }).focusable(true).cursor(vui_core::CursorIcon::Pointer).on_click(f(date));
            } else if !allowed {
                cell = cell.disabled(true);
            }
            cells.push(cell.into_element());
        }
        column()
            .gap(4.0)
            .width(7.0 * CELL + 6.0 * 2.0)
            .role(Role::Group)
            .name(format!("{} {}", month.month_name(), month.year))
            .children((header, grid(7).gap(2.0).focus_group(FocusGroup::both()).children(cells)))
            .into_element()
    }
}

impl<M: Clone + 'static> vui_runtime::IntoChildren<M> for Calendar<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}

/// The month grid's container, so the date picker can put it in a card.
pub(super) fn card<M: 'static>() -> Div<M> {
    crate::overlay::panel().padding(10.0)
}
