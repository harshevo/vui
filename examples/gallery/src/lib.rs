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
    Overlays,
    Data,
    Charts,
    Feedback,
    Effects,
    Settings,
}

impl Page {
    pub const ALL: [Page; 13] = [
        Page::Foundations,
        Page::Buttons,
        Page::Controls,
        Page::Inputs,
        Page::Lists,
        Page::Images,
        Page::Icons,
        Page::Overlays,
        Page::Data,
        Page::Charts,
        Page::Feedback,
        Page::Effects,
        Page::Settings,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Page::Foundations => "Foundations",
            Page::Buttons => "Buttons",
            Page::Controls => "Controls",
            Page::Inputs => "Inputs",
            Page::Lists => "Lists",
            Page::Images => "Images",
            Page::Icons => "Icons",
            Page::Overlays => "Overlays",
            Page::Data => "Data",
            Page::Charts => "Charts",
            Page::Feedback => "Feedback",
            Page::Effects => "Effects",
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
            Page::Overlays => icon_set::COPY,
            Page::Data => icon_set::GRID,
            Page::Charts => icon_set::BAR_CHART,
            Page::Feedback => icon_set::BELL,
            Page::Effects => icon_set::SPARKLES,
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
    /// Opens one of the overlays, or closes whichever is open.
    Open(Option<Overlay>),
    Sort(Sort),
    Density(u32),
    Context(Point),
    CloseContext,
    Toast,
    HideToast(u32),
    Tab(usize),
    SortBy(usize),
    Pick(usize),
    Fold(usize),
    Section(usize),
    Count(f64),
    Unfilter(usize),
    PageTo(usize),
    /// Opens or closes the page menu of a narrow window.
    Nav(bool),
}

/// The overlay the overlays page has open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    Menu,
    Select,
    Dialog,
}

/// How the overlays page's menu sorts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sort {
    Newest,
    Oldest,
    Name,
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
    pub overlay: Option<Overlay>,
    pub sort: Sort,
    pub density: u32,
    pub context: Option<Point>,
    /// Toasts showing, by number.
    pub toasts: Vec<u32>,
    pub toasted: u32,
    pub tab: usize,
    /// The data page's sort: column and whether it goes down.
    pub sort_by: (usize, bool),
    pub picked: usize,
    /// Tree rows that are unfolded, by index.
    pub unfolded: Vec<usize>,
    pub section: Option<usize>,
    pub count: f64,
    pub filters: Vec<&'static str>,
    pub page_no: usize,
    pub nav_open: bool,
    /// Hold everything that moves by itself still: set for screenshots, so they match from run to run.
    pub still: bool,
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
            overlay: None,
            sort: Sort::Newest,
            density: 2,
            context: None,
            toasts: Vec::new(),
            toasted: 0,
            tab: 0,
            sort_by: (0, false),
            picked: 1,
            unfolded: vec![0, 2],
            section: Some(0),
            count: 3.0,
            filters: vec!["Design", "Due this week", "Assigned to me"],
            page_no: 6,
            nav_open: false,
            still: false,
            pictures: Pictures { scenes, detail, disc, hash },
        }
    }
}

impl App for Gallery {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Go(page) => {
                self.page = page;
                self.nav_open = false;
            }
            Msg::Nav(open) => self.nav_open = open,
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
            Msg::Open(overlay) => self.overlay = overlay,
            Msg::Sort(sort) => {
                self.sort = sort;
                self.overlay = None;
            }
            Msg::Density(d) => {
                self.density = d;
                self.overlay = None;
            }
            Msg::Context(at) => self.context = Some(at),
            Msg::CloseContext => self.context = None,
            Msg::Toast => {
                self.toasted += 1;
                self.toasts.push(self.toasted);
                cx.after(std::time::Duration::from_secs(4), Msg::HideToast(self.toasted));
            }
            Msg::HideToast(n) => self.toasts.retain(|&t| t != n),
            Msg::Tab(t) => self.tab = t,
            Msg::SortBy(c) => self.sort_by = (c, self.sort_by.0 == c && !self.sort_by.1),
            Msg::Pick(r) => self.picked = r,
            Msg::Fold(i) => {
                if let Some(at) = self.unfolded.iter().position(|&u| u == i) {
                    self.unfolded.remove(at);
                } else {
                    self.unfolded.push(i);
                }
            }
            Msg::Section(i) => self.section = if self.section == Some(i) { None } else { Some(i) },
            Msg::Count(v) => self.count = v,
            Msg::Unfilter(i) => {
                if i < self.filters.len() {
                    self.filters.remove(i);
                }
            }
            Msg::PageTo(p) => self.page_no = p,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        // A narrow window trades the sidebar for a bar with a menu of the pages.
        let wide_only = |d: Div<Msg>| d.when(Breakpoint::Compact, |s| s.hidden(true));
        row().children((
            wide_only(self.sidebar()),
            wide_only(separator_vertical()),
            column().grow(1.0).min_width(0.0).children((
                self.top_bar(),
                column().grow(1.0).min_height(0.0).overflow(Overflow::Scroll).key("content").child(pages::view(self)),
            )),
        ))
    }

    fn theme(&self, _system: vui::core::Mode) -> Theme {
        if self.dark { Theme::dark() } else { Theme::light() }
    }
}

impl Gallery {
    /// The bar a narrow window shows in place of the sidebar: the page menu and the dark switch.
    fn top_bar(&self) -> Div<Msg> {
        let pages: Vec<_> = Page::ALL.into_iter().map(|page| {
                let item = menu_item(page.name()).icon(page.icon()).on_click(Msg::Go(page));
                if self.page == page { item.text_color(Tone::Accent).icon_color(Tone::Accent) } else { item }
            }).collect();
        row()
            .hidden(true)
            .when(Breakpoint::Compact, |s| s.hidden(false))
            .key("top-bar")
            .height(52.0)
            .padding_x(12.0)
            .gap(8.0)
            .align_items(Align::Center)
            .bg(Tone::Side)
            .children((
                dropdown(
                    button(self.page.name()).ghost().icon(icon_set::MENU).trailing(icon_set::CHEVRON_DOWN).key("nav").on_click(Msg::Nav(!self.nav_open)),
                    self.nav_open,
                    Msg::Nav(false),
                    pages,
                ),
                spacer(),
                icon(icon_set::MOON),
                toggle(self.dark).name("Dark mode").on_change(Msg::Dark),
            ))
    }

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
