//! The graph view in a running UI: picking, hovering, panning, live updates, and a golden sheet of a
//! knowledge graph in light and dark.

use std::rc::Rc;

use vui_core::{Fonts, Mode, Point, Theme, pt, size};
use vui_render::assert_golden;
use vui_render::testing::render_scene;
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Role, Styled, column};
use vui_viz::graph::{Edge, GraphData, Highlight, Node, graph_view};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Select(Option<u64>),
    Open(u64),
}

struct Page {
    data: Rc<GraphData>,
    selected: Option<u64>,
    highlight: Highlight,
    log: Vec<Msg>,
    dark: bool,
}

impl App for Page {
    type Msg = Msg;

    fn theme(&self, _system: Mode) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        if let Msg::Select(s) = &msg {
            self.selected = *s;
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().child(
            graph_view(self.data.clone())
                .key("g")
                .width(640.0)
                .height(420.0)
                .selected(self.selected)
                .highlight(self.highlight.clone())
                .on_select(Msg::Select)
                .on_open(Msg::Open)
                .name("Knowledge graph"),
        )
    }
}

const COMPANIES: [&str; 8] = [
    "Nimbus Cloud",
    "Helix Bio",
    "Orbit Labs",
    "Vertex Power",
    "Quanta Chips",
    "Aster Pay",
    "Lumen Retail",
    "Terra Motors",
];
const PEOPLE: [&str; 10] = [
    "A. Rao",
    "M. Chen",
    "S. Okafor",
    "L. Moreau",
    "J. Park",
    "R. Silva",
    "T. Novak",
    "E. Haddad",
    "P. Iyer",
    "D. Kim",
];
const FUNDS: [&str; 4] = [
    "Granite Capital",
    "Northwind VC",
    "Blue Ledger",
    "Sable Partners",
];
const PLACES: [&str; 4] = ["Bengaluru", "Austin", "Shenzhen", "Berlin"];

/// A small, repeatable knowledge graph: companies, the people who run them, who funds them, and where.
fn knowledge_graph() -> GraphData {
    let mut d = GraphData::new().kinds(&["Company", "Person", "Fund", "Place"]);
    let mut id = 0u64;
    let mut ids = |n: usize| -> Vec<u64> {
        (0..n)
            .map(|_| {
                id += 1;
                id
            })
            .collect()
    };
    let (co, pe, fu, pl) = (
        ids(COMPANIES.len()),
        ids(PEOPLE.len()),
        ids(FUNDS.len()),
        ids(PLACES.len()),
    );
    for (i, n) in COMPANIES.iter().enumerate() {
        d = d.node(
            Node::new(co[i], *n, 0)
                .weight(4.0 + (i % 3) as f32 * 2.0)
                .detail("Public company"),
        );
    }
    for (i, n) in PEOPLE.iter().enumerate() {
        d = d.node(Node::new(pe[i], *n, 1).weight(1.5).detail("Executive"));
    }
    for (i, n) in FUNDS.iter().enumerate() {
        d = d.node(Node::new(fu[i], *n, 2).weight(3.0).detail("Investor"));
    }
    for (i, n) in PLACES.iter().enumerate() {
        d = d.node(Node::new(pl[i], *n, 3).weight(2.0));
    }
    for i in 0..PEOPLE.len() {
        d = d.edge(Edge::new(
            pe[i],
            co[i % COMPANIES.len()],
            if i < COMPANIES.len() {
                "leads"
            } else {
                "board member"
            },
        ));
    }
    for (i, c) in co.iter().enumerate() {
        d = d.edge(Edge::new(fu[i % FUNDS.len()], *c, "invested in").weight(2.0));
        d = d.edge(Edge::new(*c, pl[(i * 3) % PLACES.len()], "based in"));
        if i + 1 < co.len() && i % 2 == 0 {
            d = d.edge(Edge::new(*c, co[i + 1], "supplies"));
        }
    }
    d = d
        .edge(Edge::new(co[0], co[4], "competes with").dashed())
        .edge(Edge::new(co[3], co[7], "acquired").weight(3.0));
    d
}

fn page() -> TestUi<Msg> {
    TestUi::with_app(
        640.0,
        420.0,
        Page {
            data: Rc::new(knowledge_graph()),
            selected: None,
            highlight: Highlight::default(),
            log: Vec::new(),
            dark: false,
        },
    )
}

fn settled() -> TestUi<Msg> {
    let mut t = page();
    t.settle();
    t
}

fn log(t: &TestUi<Msg>) -> Vec<Msg> {
    t.app::<Page>().log.clone()
}

/// Where a node is on screen, found from its label: the disc sits just left or just right of the text.
fn node_at(t: &mut TestUi<Msg>, name: &str) -> Point {
    let scene = t.scene();
    let (origin, width, height) = scene
        .commands
        .iter()
        .find_map(|c| match c {
            vui_core::Command::Text(x) if x.layout.text() == name => {
                Some((x.origin, x.layout.size().w, x.layout.size().h))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{name} is labelled in {:?}", texts(&scene)));
    let y = origin.y + height / 2.0;
    let discs = quads(&scene);
    [origin.x - 10.0, origin.x + width + 10.0]
        .into_iter()
        .map(|x| pt(x, y))
        .min_by(|a, b| nearest_disc(&discs, *a).total_cmp(&nearest_disc(&discs, *b)))
        .unwrap()
}

/// How far `p` is from the centre of the closest small round quad.
fn nearest_disc(discs: &[&vui_core::Quad], p: Point) -> f32 {
    discs
        .iter()
        .filter(|q| q.rect.w < 24.0 && q.rect.w > 5.0)
        .map(|q| q.rect.center().distance(p))
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn it_settles_and_shows_names_and_a_legend() {
    let mut t = settled();
    let shown = texts(&t.scene());
    for word in ["Company", "Person", "Fund", "Place", "Nimbus Cloud"] {
        assert!(shown.contains(&word.to_string()), "{word} in {shown:?}");
    }
    assert!(t.settle().scene().commands.len() > 20);
}

#[test]
fn clicking_a_node_picks_it_and_clicking_nothing_lets_go() {
    let mut t = settled();
    let node = node_at(&mut t, "Nimbus Cloud");
    t.click(node);
    assert_eq!(log(&t), [Msg::Select(Some(1))]);
    let shown = texts(&t.scene());
    assert!(shown.contains(&"Nimbus Cloud".to_string()));
    t.click(pt(10.0, 10.0));
    assert_eq!(log(&t).last(), Some(&Msg::Select(None)));
    t.click(pt(12.0, 12.0));
    assert_eq!(log(&t).len(), 2, "clicking nothing again says nothing");
}

#[test]
fn double_clicking_a_node_opens_it() {
    let mut t = settled();
    let node = node_at(&mut t, "Helix Bio");
    t.double_click(node);
    assert_eq!(
        log(&t),
        [Msg::Select(Some(2)), Msg::Open(2)],
        "the first click picks it, the second opens it"
    );
}

#[test]
fn hovering_a_node_shows_its_card_and_leaving_hides_it() {
    let mut t = settled();
    let node = node_at(&mut t, "Granite Capital");
    t.move_to(node);
    let shown = texts(&t.scene());
    assert!(
        shown
            .iter()
            .any(|s| s.starts_with("Fund ·") && s.contains("links")),
        "{shown:?}"
    );
    assert!(shown.contains(&"Investor".to_string()));
    t.leave();
    assert!(!texts(&t.scene()).contains(&"Investor".to_string()));
}

#[test]
fn dragging_empty_space_pans_everything_together() {
    let mut t = settled();
    let before = node_at(&mut t, "Nimbus Cloud");
    let other = node_at(&mut t, "Helix Bio");
    t.press(pt(5.0, 5.0))
        .move_to(pt(45.0, 25.0))
        .move_to(pt(85.0, 45.0))
        .release(pt(85.0, 45.0));
    let after = node_at(&mut t, "Nimbus Cloud");
    let other_after = node_at(&mut t, "Helix Bio");
    assert!(
        (after.x - before.x - 80.0).abs() < 1.5 && (after.y - before.y - 40.0).abs() < 1.5,
        "{before:?} -> {after:?}"
    );
    assert!(
        ((other_after.x - other.x) - (after.x - before.x)).abs() < 0.5,
        "they move as one"
    );
    assert_eq!(log(&t), [], "a drag is not a click");
}

#[test]
fn a_new_node_grows_in_while_the_old_ones_keep_their_places() {
    let mut t = settled();
    let (a, b) = (node_at(&mut t, "Orbit Labs"), node_at(&mut t, "Helix Bio"));
    let apart = a.distance(b);
    t.update_app::<Page, _>(|p| {
        let mut d = (*p.data).clone();
        d = d
            .node(Node::new(900, "Newly Added", 0).weight(3.0))
            .edge(Edge::new(900, 3, "partners with"));
        p.data = Rc::new(d);
    });
    // The view may ease to fit the newcomer, but the old nodes stay where they were relative to each other.
    let (a2, b2) = (node_at(&mut t, "Orbit Labs"), node_at(&mut t, "Helix Bio"));
    assert!(
        (a2.distance(b2) / apart - 1.0).abs() < 0.03,
        "{apart} -> {}",
        a2.distance(b2)
    );
    t.settle();
    assert!(texts(&t.scene()).contains(&"Newly Added".to_string()));
}

#[test]
fn something_lit_by_the_app_keeps_pulsing_until_cleared() {
    let mut t = settled();
    assert!(!t.ui.needs_frame(), "a settled graph is still");
    t.update_app::<Page, _>(|p| {
        p.highlight = Highlight {
            nodes: vec![1, 2],
            edges: vec![(1, 2)],
            pulse: vec![2],
        }
    });
    t.advance(0.2);
    assert!(t.ui.needs_frame(), "the rings keep animating");
    t.update_app::<Page, _>(|p| p.highlight = Highlight::default());
    t.settle();
    assert!(!t.ui.needs_frame());
}

#[test]
fn it_reads_as_an_image_with_a_name() {
    let mut t = page();
    let tree = t.ui.access_tree();
    let n = tree.find("Knowledge graph").expect("named").clone();
    assert_eq!(n.role, Role::Image);
    assert!(n.focusable);
}

/// A settled graph as a page would show it, with one node picked and another under the pointer.
fn sheet(dark: bool, selected: Option<u64>, hover: Option<&str>) -> vui_core::Scene {
    let page = Page {
        data: Rc::new(knowledge_graph()),
        selected,
        highlight: Highlight::default(),
        log: Vec::new(),
        dark,
    };
    let mut t: TestUi<Msg> = TestUi::with_app_scaled(size(640.0, 420.0), 2.0, page);
    t.settle();
    if let Some(name) = hover {
        let node = node_at(&mut t, name);
        t.move_to(node);
    }
    t.scene()
}

#[test]
fn a_knowledge_graph_overview_in_light() {
    let fonts = Fonts::bundled_only();
    assert_golden!(
        "graph_overview_light",
        render_scene(&sheet(false, None, None), &fonts)
    );
}

#[test]
fn a_picked_node_and_its_neighbourhood_in_light() {
    let fonts = Fonts::bundled_only();
    assert_golden!(
        "graph_picked_light",
        render_scene(&sheet(false, Some(4), None), &fonts)
    );
}

#[test]
fn a_hovered_node_in_dark() {
    let fonts = Fonts::bundled_only();
    assert_golden!(
        "graph_hover_dark",
        render_scene(&sheet(true, None, Some("Nimbus Cloud")), &fonts)
    );
}
