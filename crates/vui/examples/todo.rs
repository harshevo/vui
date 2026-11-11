//! A to-do list: a text field, a list built from data, filters and an empty state, laid out to suit
//! any window from a phone-sized one up.
//!
//! ```sh
//! cargo run -p vui --example todo
//! ```

use vui::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Draft(String),
    Add,
    Toggle(u64, bool),
    Remove(u64),
    Show(Filter),
    ClearDone,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Done,
}

pub struct Task {
    pub id: u64,
    pub text: String,
    pub done: bool,
}

pub struct Todo {
    pub draft: String,
    pub tasks: Vec<Task>,
    pub filter: Filter,
    next: u64,
}

impl Todo {
    pub fn new(tasks: &[(&str, bool)]) -> Todo {
        let mut todo = Todo { draft: String::new(), tasks: Vec::new(), filter: Filter::All, next: 0 };
        for &(text, done) in tasks {
            todo.push(text, done);
        }
        todo
    }

    fn push(&mut self, text: &str, done: bool) {
        self.tasks.push(Task { id: self.next, text: text.to_string(), done });
        self.next += 1;
    }

    fn shown(&self) -> impl Iterator<Item = &Task> {
        self.tasks.iter().filter(move |t| match self.filter {
            Filter::All => true,
            Filter::Active => !t.done,
            Filter::Done => t.done,
        })
    }
}

impl App for Todo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Draft(text) => self.draft = text,
            Msg::Add => {
                let text = self.draft.trim().to_string();
                if !text.is_empty() {
                    self.push(&text, false);
                    self.draft.clear();
                }
                // Keep typing: the field keeps the keyboard after Return or a click on Add.
                cx.focus("draft");
            }
            Msg::Toggle(id, done) => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
                    task.done = done;
                }
            }
            Msg::Remove(id) => self.tasks.retain(|t| t.id != id),
            Msg::Show(filter) => self.filter = filter,
            Msg::ClearDone => self.tasks.retain(|t| !t.done),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let left = self.tasks.iter().filter(|t| !t.done).count();
        let entry = row().gap(8.0).children((
            text_field(&self.draft)
                .key("draft")
                .placeholder("What needs doing?")
                .autofocus()
                .grow(1.0)
                .on_change(Msg::Draft)
                .on_submit(|_| Msg::Add),
            button("Add").primary().icon(icon_set::PLUS).key("add").disabled(self.draft.trim().is_empty()).on_click(Msg::Add),
        ));
        let filters = segmented(
            [(Filter::All, "All".to_string()), (Filter::Active, "Active".to_string()), (Filter::Done, "Done".to_string())],
            &self.filter,
            Msg::Show,
        );
        let rows: Vec<Element<Msg>> = self.shown().map(|t| task_row(t).into_element()).collect();
        let body = if rows.is_empty() {
            let (title, detail) = match self.filter {
                Filter::Done => ("Nothing done yet", "Finished tasks show up here."),
                _ => ("All clear", "Add a task above to get going."),
            };
            empty_state(icon_set::CHECK_CIRCLE, title, detail, ()).into_element()
        } else {
            list().key("tasks").children(rows).into_element()
        };
        let footer = row().align_items(Align::Center).gap(8.0).wrap().children((
            small(format!("{left} {} left", if left == 1 { "task" } else { "tasks" })).color(Tone::Text2).grow(1.0),
            filters,
            button("Clear done").ghost().small().key("clear").disabled(self.tasks.iter().all(|t| !t.done)).on_click(Msg::ClearDone),
        ));

        // The card is as wide as the window allows up to 560 and sits in the middle; on a narrow
        // window it fills the width, with less space around it.
        let card = card().key("card").width(pct(100.0)).max_width(560.0).padding(20.0).gap(16.0).children((title("Today"), entry, body, footer));
        column()
            .fill()
            .align_items(Align::Center)
            .padding(32.0)
            .when(Breakpoint::Compact, |s| s.padding(12.0))
            .overflow(Overflow::Scroll)
            .window_drag(true)
            .child(card)
    }
}

fn task_row(task: &Task) -> Div<Msg> {
    let id = task.id;
    row().key(id).align_items(Align::Center).gap(12.0).padding_x(8.0).height(40.0).rounded(Radius::Px(8.0)).hover_bg(Tone::Hover).children((
        checkbox(task.done).name(task.text.clone()).on_change(move |done| Msg::Toggle(id, done)),
        label(task.text.clone()).color(if task.done { Tone::Text3 } else { Tone::Text }).grow(1.0).min_width(0.0).max_lines(1),
        icon_button(icon_set::TRASH, "Remove").ghost().small().key(format!("remove-{id}")).on_click(Msg::Remove(id)),
    ))
}

#[cfg(target_os = "macos")]
fn main() {
    let start = Todo::new(&[("Read the vui guides", true), ("Build the first screen", false), ("Write a golden test", false)]);
    let window = WindowOptions::new("Todo").size(size(640.0, 560.0)).min_size(size(320.0, 360.0));
    if let Err(e) = vui::run(window, |_resources| start) {
        eprintln!("{e}");
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("vui has a window backend for macOS only so far; the app still runs headless in tests.");
}
