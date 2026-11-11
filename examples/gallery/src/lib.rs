//! The vui gallery: every stock widget in one window, in light and dark. It is the library's first
//! real app and the subject of its golden screenshots.

mod art;
mod headless;
pub mod pages;

use vui::prelude::*;
use vui::core::{ImageId, Resources};

pub use headless::Headless;

/// The pages in the sidebar.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Foundations,
    Buttons,
    Controls,
    Inputs,
    Lists,
    Images,
    Icons,
    Settings,
}

impl Page {
    pub const ALL: [Page; 8] =
        [Page::Foundations, Page::Buttons, Page::Controls, Page::Inputs, Page::Lists, Page::Images, Page::Icons, Page::Settings];

    pub fn name(self) -> &'static str {
        match self {
            Page::Foundations => "Foundations",
            Page::Buttons => "Buttons",
            Page::Controls => "Controls",
            Page::Inputs => "Inputs",
            Page::Lists => "Lists",
            Page::Images => "Images",
            Page::Icons => "Icons",
            Page::Settings => "Settings",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Page::Foundations => icon_set::LAYERS,
            Page::Buttons => icon_set::COMMAND,
            Page::Controls => icon_set::SLIDERS,
            Page::Inputs => icon_set::TERMINAL,
            Page::Lists => icon_set::LIST,
            Page::Images => icon_set::IMAGE,
            Page::Icons => icon_set::STAR,
            Page::Settings => icon_set::SETTINGS,
        }
    }
}

/// How thorough the assistant should be: three choices for the radio group.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Quick,
    Balanced,
    Thorough,
}

impl Choice {
    pub const ALL: [Choice; 3] = [Choice::Quick, Choice::Balanced, Choice::Thorough];

    pub fn name(self) -> &'static str {
        match self {
            Choice::Quick => "Quick",
            Choice::Balanced => "Balanced",
            Choice::Thorough => "Thorough",
        }
    }
}

/// The appearance choices for the segmented control.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
    System,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Light, Mode::Dark, Mode::System];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Light => "Light",
            Mode::Dark => "Dark",
            Mode::System => "System",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Go(Page),
    Dark(bool),
    Wifi(bool),
    Alerts(bool),
    Login(bool),
    Agree(bool),
    Choice(Choice),
    Mode(Mode),
    Volume(f64),
    Level(f64),
    Name(String),
    Query(String),
    Secret(String),
    Notes(String),
    Row(usize),
    Erase,
}

/// The pictures the images page shows, made once at the start.
pub struct Pictures {
    pub scenes: Vec<(art::Scene, ImageId)>,
    pub detail: ImageId,
    pub disc: ImageId,
    /// The first scene's ThumbHash: what a tile shows while its picture loads.
    pub hash: Vec<u8>,
}

/// Everything the gallery remembers.
pub struct Gallery {
    pub page: Page,
    pub dark: bool,
    pub wifi: bool,
    pub alerts: bool,
    pub login: bool,
    pub agree: CheckState,
    pub choice: Choice,
    pub mode: Mode,
    pub volume: f64,
    pub level: f64,
    pub name: String,
    pub query: String,
    pub secret: String,
    pub notes: String,
    pub row: usize,
    pub erased: u32,
    pub pictures: Pictures,
}

impl Gallery {
    pub fn new(res: &Resources, page: Page, dark: bool) -> Gallery {
        let make = |scene, w, h| res.images.insert(art::landscape(scene, w, h));
        let scenes = [art::Scene::Dusk, art::Scene::Lagoon, art::Scene::Meadow, art::Scene::Ember]
            .map(|scene| (scene, make(scene, 640, 420)))
            .to_vec();
        let hash = res.images.get(scenes[0].1).expect("just inserted").thumb_hash();
        let detail = res.images.insert(art::fine_detail(1200, 800));
        let disc = res.images.insert(art::soft_disc(96));
        Gallery {
            page,
            dark,
            wifi: true,
            alerts: false,
            login: true,
            agree: CheckState::Mixed,
            choice: Choice::Balanced,
            mode: Mode::System,
            volume: 40.0,
            level: 3.0,
            name: "Ada Lovelace".into(),
            query: String::new(),
            secret: "hunter22".into(),
            notes: "Call the dentist\nBuy oat milk\nRead the vui README".into(),
            row: 1,
            erased: 0,
            pictures: Pictures { scenes, detail, disc, hash },
        }
    }
}

impl App for Gallery {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Go(page) => self.page = page,
            Msg::Dark(dark) => self.dark = dark,
            Msg::Wifi(on) => self.wifi = on,
            Msg::Alerts(on) => self.alerts = on,
            Msg::Login(on) => self.login = on,
            Msg::Agree(on) => self.agree = on.into(),
            Msg::Choice(choice) => self.choice = choice,
            Msg::Mode(mode) => {
                self.mode = mode;
                match mode {
                    Mode::Light => self.dark = false,
                    Mode::Dark => self.dark = true,
                    Mode::System => {}
                }
            }
            Msg::Volume(v) => self.volume = v,
            Msg::Level(v) => self.level = v,
            Msg::Name(text) => self.name = text,
            Msg::Query(text) => self.query = text,
            Msg::Secret(text) => self.secret = text,
            Msg::Notes(text) => self.notes = text,
            Msg::Row(i) => self.row = i,
            Msg::Erase => self.erased += 1,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        row().children((
            self.sidebar(),
            separator_vertical(),
            column().grow(1.0).min_width(0.0).overflow(Overflow::Scroll).key("content").child(pages::view(self)),
        ))
    }

    fn theme(&self, _system: vui::core::Mode) -> Theme {
        if self.dark { Theme::dark() } else { Theme::light() }
    }
}

impl Gallery {
    fn sidebar(&self) -> Div<Msg> {
        let brand = row().align_items(Align::Center).gap(8.0).padding_x(12.0).height(32.0).children((
            div().square(20.0).rounded(Radius::Px(6.0)).bg(Tone::Accent),
            label("vui").weight(600),
        ));
        let pages = list().key("pages").children(Page::ALL.map(|page| {
            nav_item(page.icon(), page.name()).key(page.name()).selected(self.page == page).on_click(Msg::Go(page))
        }));
        let dark = row().align_items(Align::Center).gap(8.0).padding_x(12.0).height(32.0).children((
            icon(icon_set::MOON),
            label("Dark").color(Tone::Text2).grow(1.0),
            toggle(self.dark).key("dark").name("Dark mode").on_change(Msg::Dark),
        ));
        sidebar().key("sidebar").children((brand, pages, spacer(), dark))
    }
}
