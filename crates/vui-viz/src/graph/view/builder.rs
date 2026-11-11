//! Building a graph view: `graph_view` and the methods that configure it.

use super::*;

pub struct GraphView<M: 'static> {
    pub(crate) base: Base<M>,
    pub(crate) widget: GraphWidget<M>,
}

impl<M: 'static> GraphView<M> {
    /// The picked node, by id. The widget draws it ringed and keeps its neighbourhood bright.
    pub fn selected(mut self, id: Option<u64>) -> Self {
        self.widget.selected = id;
        self
    }

    pub fn highlight(mut self, highlight: Highlight) -> Self {
        self.widget.highlight = highlight;
        self
    }

    /// Shows the graph as it was at this time (seconds since 1970): nodes and edges outside their span
    /// are left out.
    pub fn as_of(mut self, t: Option<f64>) -> Self {
        self.widget.as_of = t;
        self
    }

    /// Sends the id of a node clicked, or `None` for a click on nothing or Escape.
    pub fn on_select(mut self, f: impl Fn(Option<u64>) -> M + 'static) -> Self {
        self.widget.on_select = Some(Rc::new(f));
        self
    }

    /// Sends the id of a node double-clicked.
    pub fn on_open(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.widget.on_open = Some(Rc::new(f));
        self
    }

    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }
}

/// A graph that fills the room it is given. Share the data with `Rc`: the view only looks at it again when
/// it is a different `Rc` holding something different.
pub fn graph_view<M: 'static>(data: Rc<GraphData>) -> GraphView<M> {
    let view = GraphView {
        base: Base::new(),
        widget: GraphWidget {
            data,
            selected: None,
            highlight: Highlight::default(),
            as_of: None,
            label: String::new(),
            on_select: None,
            on_open: None,
        },
    };
    view.focus_ring(true).cursor(CursorIcon::Grab)
}
