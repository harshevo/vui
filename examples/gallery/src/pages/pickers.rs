//! Pickers and composites: combobox, tags, dates, times, colours, steps, a toolbar, a carousel, a
//! hover card, the command palette and drawers. The page keeps its own state and messages.

use super::*;

const CITIES: [&str; 10] = ["Amsterdam", "Barcelona", "Berlin", "Copenhagen", "Lisbon", "London", "Los Angeles", "Lyon", "Oslo", "Tokyo"];

/// What the page remembers.
pub struct Pickers {
    pub city: Picker,
    pub tags: Vec<String>,
    pub draft: String,
    pub due: Option<Date>,
    pub due_picker: DatePicker,
    pub month: Date,
    pub time: (u8, u8),
    pub color: Color,
    pub step: usize,
    pub slide: usize,
    pub peek: bool,
    pub palette: Picker,
    pub ran: Option<String>,
    pub drawer: Option<Edge>,
    pub bold: bool,
}

impl Default for Pickers {
    fn default() -> Pickers {
        let day = Date::new(2026, 11, 18).expect("a real day");
        Pickers {
            city: Picker::new(),
            tags: vec!["design".into(), "rust".into()],
            draft: String::new(),
            due: Some(day),
            due_picker: DatePicker::default(),
            month: day.first_of_month(),
            time: (9, 30),
            color: Color::hex(0x4f5bd5),
            step: 2,
            slide: 0,
            peek: false,
            palette: Picker::new(),
            ran: None,
            drawer: None,
            bold: true,
        }
    }
}

/// The page's messages, inside [`Msg::Pickers`].
#[derive(Clone, Debug, PartialEq)]
pub enum PickMsg {
    City(PickerEvent),
    Tags(TagEvent),
    Due(DateEvent),
    Day(Date),
    Month(Date),
    Time(u8, u8),
    Color(Color),
    Step(usize),
    Slide(usize),
    Peek(bool),
    Palette(PickerEvent),
    Drawer(Option<Edge>),
    Bold,
}

fn commands() -> Vec<Command> {
    vec![
        command("New file").group("File").shortcut("⌘N").icon(icon_set::PLUS),
        command("Open…").group("File").shortcut("⌘O").icon(icon_set::FOLDER),
        command("Toggle dark mode").group("View").icon(icon_set::MOON).keywords("theme appearance"),
        command("Show sidebar").group("View").shortcut("⌘\\").icon(icon_set::SIDEBAR),
        command("Preferences").group("App").shortcut("⌘,").icon(icon_set::SETTINGS).keywords("settings options"),
    ]
}

impl Pickers {
    pub fn update(&mut self, msg: PickMsg) {
        match msg {
            PickMsg::City(e) => {
                self.city.apply(e);
            }
            PickMsg::Tags(TagEvent::Draft(d)) => self.draft = d,
            PickMsg::Tags(TagEvent::Add(t)) => {
                if !self.tags.contains(&t) {
                    self.tags.push(t);
                }
                self.draft.clear();
            }
            PickMsg::Tags(TagEvent::Remove(i)) => {
                if i < self.tags.len() {
                    self.tags.remove(i);
                }
            }
            PickMsg::Due(e) => {
                if let Some(day) = self.due_picker.apply(e) {
                    self.due = Some(day);
                }
            }
            PickMsg::Day(d) => self.due = Some(d),
            PickMsg::Month(m) => self.month = m,
            PickMsg::Time(h, m) => self.time = (h, m),
            PickMsg::Color(c) => self.color = c,
            PickMsg::Step(s) => self.step = s,
            PickMsg::Slide(s) => self.slide = s,
            PickMsg::Peek(p) => self.peek = p,
            PickMsg::Palette(e) => {
                if let Some(i) = self.palette.apply(e) {
                    self.ran = commands().get(i).map(|c| c.title.clone());
                    self.palette.reset();
                }
            }
            PickMsg::Drawer(d) => self.drawer = d,
            PickMsg::Bold => self.bold = !self.bold,
        }
    }
}

fn slide(name: &str, tone: Tone) -> Div<Msg> {
    column().height(140.0).padding(20.0).padding_x(56.0).justify(Justify::End).bg(Fill::Tone(tone, 0.16)).children((title(name), small("Swipe, click the arrows, or use ← →").color(Tone::Text2)))
}

pub fn view(g: &Gallery) -> Div<Msg> {
    let p = &g.pickers;
    let cities: Vec<String> = CITIES.iter().map(|c| c.to_string()).collect();
    let tools = toolbar((
        icon_button(icon_set::BOLD, "Bold").on_click(Msg::Pickers(PickMsg::Bold)).bg(if p.bold { Fill::from(Tone::Wash) } else { Fill::NONE }),
        icon_button(icon_set::ITALIC, "Italic"),
        toolbar_separator(),
        icon_button(icon_set::LINK, "Link"),
        icon_button(icon_set::IMAGE, "Picture"),
    ))
    .border(1.0, Tone::Line)
    .rounded(Radius::Card)
    .align_self(Align::Start);
    let profile = panel().padding(14.0).width(240.0).gap(6.0).children((label("Ada Lovelace").weight(600), small("Wrote the first program, for an engine that was never built.").color(Tone::Text2)));
    let opened = p.drawer.map(|edge| {
        let body = (title("Filters"), small("A drawer holds what doesn't need its own page.").color(Tone::Text2), button("Done").primary().on_click(Msg::Pickers(PickMsg::Drawer(None))));
        match edge {
            Edge::Bottom => sheet(Msg::Pickers(PickMsg::Drawer(None)), body),
            edge => drawer(edge, Msg::Pickers(PickMsg::Drawer(None)), body),
        }
    });
    page(
        "Pickers",
        (
            section(
                "Combobox and tags",
                flow().align_items(Align::Start).children((
                    div().width(220.0).key("city").child(combobox(&p.city, &cities, |e| Msg::Pickers(PickMsg::City(e))).placeholder("City")),
                    tag_input(&p.tags, &p.draft, |e| Msg::Pickers(PickMsg::Tags(e))).width(300.0).key("tags"),
                )),
            ),
            section(
                "Dates and times",
                flow().align_items(Align::Start).gap(24.0).children((
                    calendar(p.month)
                        .selected(p.due)
                        .today(Date::new(2026, 11, 11).expect("a real day"))
                        .on_pick(|d| Msg::Pickers(PickMsg::Day(d)))
                        .on_month(|d| Msg::Pickers(PickMsg::Month(d))),
                    column().gap(16.0).children((
                        captioned("Date picker", date_picker(p.due, &p.due_picker, |e| Msg::Pickers(PickMsg::Due(e)))),
                        captioned("Time", time_field(p.time.0, p.time.1, 5, |h, m| Msg::Pickers(PickMsg::Time(h, m)))),
                    )),
                )),
            ),
            section("Colour", color_picker(p.color, |c| Msg::Pickers(PickMsg::Color(c)))),
            section("Stepper", stepper(["Cart", "Address", "Payment", "Review"], p.step).on_select(|s| Msg::Pickers(PickMsg::Step(s)))),
            section("Toolbar", tools),
            section(
                "Carousel",
                carousel(
                    p.slide,
                    (slide("Welcome", Tone::Accent), slide("Your inbox", Tone::Purple), slide("You're set", Tone::Success)),
                    |s| Msg::Pickers(PickMsg::Slide(s)),
                )
                .max_width(480.0),
            ),
            section(
                "Hover card, palette and drawers",
                flow().children((
                    hover_card(button("Ada Lovelace").ghost().icon(icon_set::USER), p.peek, |h| Msg::Pickers(PickMsg::Peek(h)), profile, Side::Below),
                    button("Command palette").icon(icon_set::COMMAND).key("palette").on_click(Msg::Pickers(PickMsg::Palette(PickerEvent::Open(true)))),
                    button("Drawer").on_click(Msg::Pickers(PickMsg::Drawer(Some(Edge::Right)))),
                    button("Sheet").on_click(Msg::Pickers(PickMsg::Drawer(Some(Edge::Bottom)))),
                    p.ran.as_ref().map(|r| small(format!("Ran “{r}”")).color(Tone::Text2)),
                )),
            ),
            command_palette(&p.palette, &commands(), |e| Msg::Pickers(PickMsg::Palette(e))),
            opened,
        ),
    )
}
