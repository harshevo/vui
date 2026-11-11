//! Steppers: where a person is in a task of several steps, such as checkout or onboarding.

use std::rc::Rc;

use vui_core::{Fill, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Element, IntoElement, Justify, Parent, Radius, Role, Styled, column, div, row, small};

use crate::display::icon::icon;

/// `steps` in a row joined by lines: the ones before `current` ticked, `current` ringed, the rest
/// waiting. Give it `on_select` to let people go back to a step they have done.
///
/// ```
/// use vui_widgets::stepper;
///
/// let checkout = stepper::<()>(["Cart", "Address", "Payment", "Review"], 2);
/// ```
pub fn stepper<M: 'static>(steps: impl IntoIterator<Item = impl Into<String>>, current: usize) -> Stepper<M> {
    Stepper { steps: steps.into_iter().map(Into::into).collect(), current, on_select: None }
}

/// See [`stepper`].
pub struct Stepper<M: 'static> {
    steps: Vec<String>,
    current: usize,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
}

impl<M: 'static> Stepper<M> {
    /// Sent with the index of a finished step when it is clicked.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

fn marker<M: 'static>(i: usize, current: usize) -> Div<M> {
    let done = i < current;
    let here = i == current;
    let dot = div().square(26.0).rounded(Radius::Pill).align_items(Align::Center).justify(Justify::Center).transition(Spring::SNAPPY);
    if done {
        dot.bg(Tone::Accent).child(icon(set::CHECK).square(14.0).color(Tone::OnAccent))
    } else {
        let ring = if here { Fill::from(Tone::Accent) } else { Fill::from(Tone::Line) };
        let text = if here { Tone::Accent } else { Tone::Text3 };
        dot.border(if here { 2.0 } else { 1.0 }, ring).child(small((i + 1).to_string()).weight(600).color(text))
    }
}

impl<M: Clone + 'static> IntoElement<M> for Stepper<M> {
    fn into_element(self) -> Element<M> {
        let Stepper { steps, current, on_select } = self;
        let n = steps.len();
        let mut children: Vec<Element<M>> = Vec::with_capacity(n * 2);
        for (i, name) in steps.into_iter().enumerate() {
            let color = if i <= current { Tone::Text } else { Tone::Text3 };
            let mut step = column()
                .gap(6.0)
                .align_items(Align::Center)
                .role(Role::ListItem)
                .name(format!("Step {} of {n}: {name}{}", i + 1, if i < current { ", done" } else if i == current { ", current" } else { "" }))
                .children((marker(i, current), small(name).weight(if i == current { 600 } else { 400 }).color(color)));
            if let Some(f) = on_select.as_ref().filter(|_| i < current) {
                step = step.cursor(vui_core::CursorIcon::Pointer).focusable(true).rounded(Radius::Control).on_click(f(i));
            }
            children.push(step.into_element());
            if i + 1 < n {
                let line = Fill::from(if i < current { Tone::Accent } else { Tone::Line });
                children.push(div().height(2.0).grow(1.0).min_width(16.0).margin_top(12.0).rounded(Radius::Pill).bg(line).transition(Spring::SMOOTH).into_element());
            }
        }
        row().gap(8.0).align_items(Align::Start).role(Role::List).children(children).into_element()
    }
}

impl<M: Clone + 'static> vui_runtime::IntoChildren<M> for Stepper<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}
