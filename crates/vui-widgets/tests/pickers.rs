//! Comboboxes, tag inputs, the command palette, calendars, date pickers, the colour picker and
//! carousels, driven the way a person would: clicks, typing and keys.

use vui_core::{Color, NamedKey};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Styled, column, div};
use vui_widgets::{
    Date, DateEvent, DatePicker, Picker, PickerEvent, TagEvent, calendar, carousel, color_picker, combobox, command, command_palette,
    date_picker, tag_input,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    City(PickerEvent),
    Tags(TagEvent),
    Palette(PickerEvent),
    Day(Date),
    Month(Date),
    Due(DateEvent),
    Color(Color),
    Slide(usize),
}

const CITIES: [&str; 5] = ["Berlin", "Lisbon", "London", "Los Angeles", "Oslo"];

#[derive(Default)]
struct Form {
    city: Picker,
    chosen: Option<usize>,
    tags: Vec<String>,
    draft: String,
    palette: Picker,
    ran: Option<usize>,
    day: Option<Date>,
    month: Option<Date>,
    due: Option<Date>,
    due_picker: DatePicker,
    color: Option<Color>,
    slide: usize,
    /// Which widget the view shows, so each test sees only its own.
    show: &'static str,
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::City(e) => self.chosen = self.city.apply(e).or(self.chosen),
            Msg::Tags(TagEvent::Draft(d)) => self.draft = d,
            Msg::Tags(TagEvent::Add(t)) => {
                self.tags.push(t);
                self.draft.clear();
            }
            Msg::Tags(TagEvent::Remove(i)) => {
                self.tags.remove(i);
            }
            Msg::Palette(e) => {
                if let Some(i) = self.palette.apply(e) {
                    self.ran = Some(i);
                    self.palette.reset();
                }
            }
            Msg::Day(d) => self.day = Some(d),
            Msg::Month(m) => self.month = Some(m),
            Msg::Due(e) => self.due = self.due_picker.apply(e).or(self.due),
            Msg::Color(c) => self.color = Some(c),
            Msg::Slide(s) => self.slide = s,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let cities: Vec<String> = CITIES.iter().map(|c| c.to_string()).collect();
        let nov = Date::new(2026, 11, 1).unwrap();
        let body = match self.show {
            "city" => div().key("city").width(240.0).child(combobox(&self.city, &cities, Msg::City)).into_element(),
            "tags" => tag_input(&self.tags, &self.draft, Msg::Tags).key("tags").width(300.0).into_element(),
            "palette" => command_palette(
                &self.palette,
                &[command("New file").group("File"), command("Toggle dark mode").group("View"), command("Quit")],
                Msg::Palette,
            )
            .into_element(),
            "calendar" => calendar(self.month.unwrap_or(nov)).selected(self.day).on_pick(Msg::Day).on_month(Msg::Month).into_element(),
            "due" => div().key("due").child(date_picker(self.due, &self.due_picker, Msg::Due)).into_element(),
            "color" => color_picker(self.color.unwrap_or(Color::hex(0x808080)), Msg::Color).into_element(),
            _ => carousel(self.slide, (div().key("a").height(40.0), div().key("b").height(40.0), div().key("c").height(40.0)), Msg::Slide).into_element(),
        };
        column().padding(20.0).child(body)
    }
}

fn form(show: &'static str) -> TestUi<Msg> {
    TestUi::with_app(600.0, 600.0, Form { show, ..Form::default() })
}

#[test]
fn a_combobox_filters_as_it_is_typed_in_and_picks_with_the_keys() {
    let mut t = form("city");
    t.click_key("city").type_text("lo");
    t.settle();
    let shown = texts(&t.scene());
    assert!(t.app::<Form>().city.open, "{:?}", t.app::<Form>().city);
    assert!(shown.contains(&"London".to_string()) && shown.contains(&"Los Angeles".to_string()), "{shown:?}");
    assert!(!shown.contains(&"Berlin".to_string()), "Berlin has no o after its l");
    // Focus stayed in the field: typing goes on narrowing.
    t.type_text("n");
    // "lon": London first, then Lisbon and Los Angeles. Down, Down, Up lands on the second.
    t.key(NamedKey::ArrowDown).key(NamedKey::ArrowDown).key(NamedKey::ArrowUp).key(NamedKey::Enter);
    let app = t.app::<Form>();
    assert!(!app.city.open);
    assert_eq!(app.chosen, Some(1));
    assert_eq!(app.city.query, "Lisbon");
}

#[test]
fn escape_closes_the_suggestions() {
    let mut t = form("city");
    t.click_key("city").type_text("b");
    assert!(t.app::<Form>().city.open);
    t.key(NamedKey::Escape);
    assert!(!t.app::<Form>().city.open);
}

#[test]
fn a_tag_input_adds_on_return_and_comma_and_removes_with_backspace() {
    let mut t = form("tags");
    t.click_key("tags").type_text("rust").key(NamedKey::Enter);
    t.type_text("ui,");
    assert_eq!(t.app::<Form>().tags, vec!["rust", "ui"]);
    assert_eq!(t.app::<Form>().draft, "");
    t.key(NamedKey::Backspace);
    assert_eq!(t.app::<Form>().tags, vec!["rust"]);
}

#[test]
fn the_command_palette_runs_what_was_typed() {
    let mut t = form("palette");
    t.ui.send(Msg::Palette(PickerEvent::Open(true)));
    t.settle();
    let shown = texts(&t.scene());
    assert!(shown.contains(&"File".to_string()) && shown.contains(&"View".to_string()), "groups head the list: {shown:?}");
    t.type_text("dark").key(NamedKey::Enter);
    assert_eq!(t.app::<Form>().ran, Some(1));
    assert!(!t.app::<Form>().palette.open);
}

#[test]
fn a_calendar_picks_days_and_turns_months() {
    let mut t = form("calendar");
    t.click_key(20usize);
    assert_eq!(t.app::<Form>().day, Date::new(2026, 11, 20));
    let shown = texts(&t.scene());
    assert!(shown.contains(&"November 2026".to_string()));
    assert!(shown.contains(&"30".to_string()) && !shown.contains(&"31".to_string()), "November has 30 days");
}

#[test]
fn a_date_picker_opens_a_calendar_and_closes_on_a_pick() {
    let mut t = form("due");
    t.click_key("due");
    assert!(t.app::<Form>().due_picker.open);
    t.click_key(14usize);
    let app = t.app::<Form>();
    assert!(!app.due_picker.open);
    assert_eq!(app.due.map(|d| d.day), Some(14));
}

#[test]
fn the_colour_square_moves_with_the_keys() {
    let mut t = form("color");
    t.tab().key(NamedKey::ArrowRight);
    let [_, s, v] = t.app::<Form>().color.expect("a colour was sent").to_hsv();
    assert!((s - 0.01).abs() < 0.005, "saturation {s}");
    assert!((v - 0.5).abs() < 0.01, "brightness kept: {v}");
}

#[test]
fn a_carousel_moves_with_the_arrows_and_wraps() {
    let mut t = form("carousel");
    t.click_key("vui-carousel-stage");
    t.key(NamedKey::ArrowLeft);
    assert_eq!(t.app::<Form>().slide, 2);
    t.key(NamedKey::ArrowRight);
    assert_eq!(t.app::<Form>().slide, 0);
}
