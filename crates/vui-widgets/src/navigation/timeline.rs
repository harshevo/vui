//! A timeline: dated steps along a line, the ones passed ticked, the one now ringed, the rest to come.
//! It is the stepper's cousin for things that happen on dates (an offer opening, its allotment, the
//! listing) rather than steps a person takes, so each step carries a date and may carry a note.

use vui_core::{Fill, Spring, TextAlign, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Element, IntoChildren, IntoElement, Justify, Parent, Radius, Role, Styled, column, div, label, row, small};

use crate::display::icon::icon;

/// Where a step is in time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum When {
    Done,
    Now,
    Next,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub title: String,
    /// Already written for people: "Mon, 24 Nov", "T+3".
    pub date: String,
    pub note: String,
    pub when: When,
}

impl Step {
    pub fn new(title: &str, date: &str, when: When) -> Step {
        Step { title: title.to_string(), date: date.to_string(), note: String::new(), when }
    }

    /// A line under the date, in the vertical layout.
    pub fn note(mut self, note: &str) -> Step {
        self.note = note.to_string();
        self
    }
}

/// Steps along a line, across by default. `.vertical()` stacks them for a narrow place.
///
/// ```
/// use vui_widgets::{Step, When, timeline};
///
/// let offer = timeline::<()>([Step::new("Opens", "Mon, 24 Nov", When::Done), Step::new("Closes", "Wed, 26 Nov", When::Now), Step::new("Listing", "Mon, 1 Dec", When::Next)]);
/// ```
pub fn timeline<M: 'static>(steps: impl IntoIterator<Item = Step>) -> Timeline<M> {
    Timeline { steps: steps.into_iter().collect(), vertical: false, _m: std::marker::PhantomData }
}

pub struct Timeline<M: 'static> {
    steps: Vec<Step>,
    vertical: bool,
    _m: std::marker::PhantomData<M>,
}

impl<M: 'static> Timeline<M> {
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
}

const MARKER: f32 = 20.0;
const LINE: f32 = 2.0;

fn marker<M: 'static>(when: When) -> Div<M> {
    let dot = div().square(MARKER).rounded(Radius::Pill).align_items(Align::Center).justify(Justify::Center).transition(Spring::SNAPPY);
    match when {
        When::Done => dot.bg(Tone::Accent).child(icon(set::CHECK).square(12.0).color(Tone::OnAccent)),
        When::Now => dot.border(2.0, Fill::from(Tone::Accent)).child(div().square(8.0).rounded(Radius::Pill).bg(Tone::Accent)),
        When::Next => dot.border(1.5, Fill::Tone(Tone::Text3, 0.7)),
    }
}

fn line<M: 'static>(lit: bool) -> Div<M> {
    div().bg(if lit { Fill::from(Tone::Accent) } else { Fill::from(Tone::Line) }).transition(Spring::SMOOTH)
}

fn words(step: &Step, i: usize, n: usize) -> String {
    let state = match step.when {
        When::Done => ", done",
        When::Now => ", now",
        When::Next => "",
    };
    format!("Step {} of {n}: {}, {}{state}", i + 1, step.title, step.date)
}

fn across<M: Clone + 'static>(steps: Vec<Step>) -> Element<M> {
    let n = steps.len();
    let reached = |i: usize| steps.get(i).is_some_and(|s| s.when == When::Done);
    let columns: Vec<Element<M>> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // Each half-line is lit when the step it leads away from is done, so the line fills as dates pass.
            let left = if i == 0 { div().grow(1.0).height(LINE) } else { line(reached(i - 1)).grow(1.0).height(LINE) };
            let right = if i + 1 == n { div().grow(1.0).height(LINE) } else { line(reached(i)).grow(1.0).height(LINE) };
            let top = row().align_items(Align::Center).fill_width().child(left).child(marker(s.when)).child(right);
            let weight = if s.when == When::Now { 600 } else { 500 };
            let words_below = column()
                .gap(2.0)
                .align_items(Align::Center)
                .child(label(s.title.clone()).weight(weight).align_text(TextAlign::Center))
                .child(small(s.date.clone()).color(if s.when == When::Next { Tone::Text2 } else { Tone::Text }).align_text(TextAlign::Center));
            column().grow(1.0).basis(0.0).min_width(72.0).gap(10.0).align_items(Align::Center).role(Role::ListItem).name(words(s, i, n)).child(top).child(words_below).into_element()
        })
        .collect();
    row().role(Role::List).children(columns).into_element()
}

fn down<M: Clone + 'static>(steps: Vec<Step>) -> Element<M> {
    let n = steps.len();
    let rows: Vec<Element<M>> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let weight = if s.when == When::Now { 600 } else { 500 };
            let rail = column().align_items(Align::Center).child(marker(s.when)).child(if i + 1 == n { div().height(0.0) } else { line(s.when == When::Done).width(LINE).grow(1.0).min_height(16.0) });
            let mut text = column()
                .gap(2.0)
                .padding_bottom(if i + 1 == n { 0.0 } else { 16.0 })
                .child(row().gap(8.0).align_items(Align::Baseline).child(label(s.title.clone()).weight(weight)).child(small(s.date.clone()).color(Tone::Text2)));
            if !s.note.is_empty() {
                text = text.child(small(s.note.clone()).color(Tone::Text2));
            }
            row().gap(12.0).align_items(Align::Stretch).role(Role::ListItem).name(words(s, i, n)).child(rail).child(text).into_element()
        })
        .collect();
    column().role(Role::List).children(rows).into_element()
}

impl<M: Clone + 'static> IntoElement<M> for Timeline<M> {
    fn into_element(self) -> Element<M> {
        if self.vertical { down(self.steps) } else { across(self.steps) }
    }
}

impl<M: Clone + 'static> IntoChildren<M> for Timeline<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}
