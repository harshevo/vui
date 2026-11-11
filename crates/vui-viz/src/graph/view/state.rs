//! What the graph widget keeps between frames: the layout, the camera, the drag in progress and what
//! has grown in.

use super::*;

pub(crate) enum Drag {
    Pan {
        last: Point,
        start: Point,
        moved: bool,
    },
    Node {
        index: usize,
        start: Point,
        moved: bool,
    },
}

pub struct GraphState {
    pub(crate) graph: Compiled,
    pub(crate) layout: Layout,
    pub(crate) grown: Vec<f32>,
    pub(crate) lit: Lit,
    /// What is on screen. Paint eases it toward where the camera wants to be, so it lives in a cell.
    pub(crate) shown: Cell<Option<Camera>>,
    pub(crate) last_paint: Cell<f32>,
    pub(crate) easing: Cell<bool>,
    pub(crate) goal: Camera,
    /// Keep everything in view as the layout moves, until the user takes the camera.
    pub(crate) follow: bool,
    pub(crate) clock: f32,
    pub(crate) pointer: Option<Point>,
    pub(crate) hover: Option<usize>,
    pub(crate) drag: Option<Drag>,
}

impl GraphState {
    pub(super) fn new(data: &GraphData, highlight: &Highlight) -> GraphState {
        let graph = Compiled::new(data);
        let mut layout = Layout::new(data.nodes.len(), &graph.pairs());
        // Let it get most of the way before the first frame, so the graph does not open as a ball.
        for _ in 0..(300_000 / data.nodes.len().max(1)).clamp(10, 120) {
            layout.step();
        }
        let lit = resolve(&graph, highlight);
        GraphState {
            grown: vec![0.0; data.nodes.len()],
            graph,
            layout,
            lit,
            shown: Cell::new(None),
            last_paint: Cell::new(0.0),
            easing: Cell::new(false),
            goal: Camera::HOME,
            follow: true,
            clock: 0.0,
            pointer: None,
            hover: None,
            drag: None,
        }
    }

    /// Takes the camera from the fit-to-view mode so it can be moved by hand.
    pub(super) fn take_camera(&mut self) {
        if self.follow {
            self.goal = self.shown.get().unwrap_or(Camera::HOME);
            self.follow = false;
        }
    }

    /// Moves the camera at once, as direct manipulation should.
    pub(super) fn snap(&mut self) {
        self.shown.set(Some(self.goal));
    }
}

pub(crate) fn resolve(graph: &Compiled, h: &Highlight) -> Lit {
    let idx = |id: &u64| graph.index.get(id).copied();
    let mut lit = Lit {
        nodes: h.nodes.iter().filter_map(idx).collect(),
        links: Default::default(),
        pulse: h.pulse.iter().filter_map(idx).collect(),
    };
    for (a, b) in &h.edges {
        if let (Some(a), Some(b)) = (idx(a), idx(b)) {
            lit.nodes.insert(a);
            lit.nodes.insert(b);
            for (n, (x, y, _)) in graph.links.iter().enumerate() {
                if (*x, *y) == (a, b) || (*x, *y) == (b, a) {
                    lit.links.insert(n);
                }
            }
        }
    }
    lit
}
