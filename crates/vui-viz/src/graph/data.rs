//! What a graph view draws: nodes and edges, in plain data with no ties to where they came from. Anything
//! that can say who is connected to whom can be shown: the knowledge base, a supply chain, a org chart.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Any number that is unique within the graph; edges refer to nodes by it.
    pub id: u64,
    pub label: String,
    /// An index into [`GraphData::kinds`]: what sort of thing this is. Decides how the node looks.
    pub kind: u8,
    /// How much this matters, more than 0. Sets the size and decides which labels survive when zoomed out.
    pub weight: f32,
    /// A line shown under the label when the node is hovered.
    pub detail: String,
    /// The span of time the node is true for, in seconds since 1970: open at either end when `None`.
    pub since: Option<f64>,
    pub until: Option<f64>,
}

impl Node {
    pub fn new(id: u64, label: impl Into<String>, kind: u8) -> Node {
        Node {
            id,
            label: label.into(),
            kind,
            weight: 1.0,
            detail: String::new(),
            since: None,
            until: None,
        }
    }

    pub fn weight(mut self, weight: f32) -> Node {
        self.weight = weight.max(f32::MIN_POSITIVE);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Node {
        self.detail = detail.into();
        self
    }

    pub fn during(mut self, since: Option<f64>, until: Option<f64>) -> Node {
        (self.since, self.until) = (since, until);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: u64,
    pub to: u64,
    /// What the link says: "acquired", "supplies", "sits on the board of".
    pub label: String,
    pub weight: f32,
    /// Drawn broken: a link nobody is sure of.
    pub dashed: bool,
    pub since: Option<f64>,
    pub until: Option<f64>,
}

impl Edge {
    pub fn new(from: u64, to: u64, label: impl Into<String>) -> Edge {
        Edge {
            from,
            to,
            label: label.into(),
            weight: 1.0,
            dashed: false,
            since: None,
            until: None,
        }
    }

    pub fn weight(mut self, weight: f32) -> Edge {
        self.weight = weight.max(f32::MIN_POSITIVE);
        self
    }

    pub fn dashed(mut self) -> Edge {
        self.dashed = true;
        self
    }

    pub fn during(mut self, since: Option<f64>, until: Option<f64>) -> Edge {
        (self.since, self.until) = (since, until);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphData {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Names of the kinds of node, for the legend, by kind index.
    pub kinds: Vec<String>,
}

impl GraphData {
    pub fn new() -> GraphData {
        GraphData::default()
    }

    pub fn kinds(mut self, names: &[&str]) -> GraphData {
        self.kinds = names.iter().map(|n| (*n).to_string()).collect();
        self
    }

    pub fn node(mut self, node: Node) -> GraphData {
        self.nodes.push(node);
        self
    }

    pub fn edge(mut self, edge: Edge) -> GraphData {
        self.edges.push(edge);
        self
    }
}

/// Whether something that is true from `since` until `until` holds at `t`. With no `t`, everything does.
pub fn alive(since: Option<f64>, until: Option<f64>, t: Option<f64>) -> bool {
    match t {
        None => true,
        Some(t) => since.is_none_or(|s| s <= t) && until.is_none_or(|u| t < u),
    }
}

/// A graph worked out for drawing: nodes by position in the list, edges as index pairs, neighbours of each.
#[derive(Clone, Debug, Default)]
pub struct Compiled {
    pub index: HashMap<u64, usize>,
    /// Each drawable edge: its two node indexes and its place in `GraphData::edges`.
    pub links: Vec<(usize, usize, usize)>,
    /// For each node: the (neighbour, link) pairs touching it.
    pub around: Vec<Vec<(usize, usize)>>,
    /// Nodes from the most to the least worth labelling.
    pub order: Vec<usize>,
    pub heaviest: f32,
}

impl Compiled {
    /// Edges that name a node the graph does not have are skipped, and so are repeats of the same id.
    pub fn new(data: &GraphData) -> Compiled {
        let mut index = HashMap::with_capacity(data.nodes.len());
        for (i, n) in data.nodes.iter().enumerate() {
            index.entry(n.id).or_insert(i);
        }
        let mut links = Vec::with_capacity(data.edges.len());
        let mut around = vec![Vec::new(); data.nodes.len()];
        for (e, edge) in data.edges.iter().enumerate() {
            if let (Some(&a), Some(&b)) = (index.get(&edge.from), index.get(&edge.to))
                && a != b
            {
                around[a].push((b, links.len()));
                around[b].push((a, links.len()));
                links.push((a, b, e));
            }
        }
        let importance = |i: usize| data.nodes[i].weight * (1.0 + around[i].len() as f32).sqrt();
        let mut order: Vec<usize> = (0..data.nodes.len()).collect();
        order.sort_by(|&a, &b| importance(b).total_cmp(&importance(a)).then(a.cmp(&b)));
        let heaviest = data
            .nodes
            .iter()
            .map(|n| n.weight)
            .fold(f32::MIN_POSITIVE, f32::max);
        Compiled {
            index,
            links,
            around,
            order,
            heaviest,
        }
    }

    pub fn pairs(&self) -> Vec<(usize, usize)> {
        self.links.iter().map(|&(a, b, _)| (a, b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GraphData {
        GraphData::new()
            .kinds(&["Company", "Person"])
            .node(Node::new(10, "Acme", 0).weight(5.0))
            .node(Node::new(20, "Bo", 1))
            .node(Node::new(30, "Cy", 1).weight(2.0))
            .edge(Edge::new(10, 20, "employs"))
            .edge(Edge::new(10, 30, "employs"))
            .edge(Edge::new(10, 99, "owns"))
            .edge(Edge::new(20, 20, "knows"))
    }

    #[test]
    fn edges_to_nothing_and_to_themselves_are_left_out() {
        let c = Compiled::new(&sample());
        assert_eq!(c.links.len(), 2);
        assert_eq!(c.pairs(), [(0, 1), (0, 2)]);
        assert_eq!(c.around[0].len(), 2);
        assert_eq!(c.around[1], [(0, 0)]);
    }

    #[test]
    fn labels_go_first_to_what_matters_most() {
        let c = Compiled::new(&sample());
        assert_eq!(c.order, [0, 2, 1]);
        assert_eq!(c.heaviest, 5.0);
    }

    #[test]
    fn a_node_id_given_twice_means_the_first() {
        let d = GraphData::new()
            .node(Node::new(1, "a", 0))
            .node(Node::new(1, "b", 0))
            .node(Node::new(2, "c", 0))
            .edge(Edge::new(1, 2, "x"));
        let c = Compiled::new(&d);
        assert_eq!(c.index[&1], 0);
        assert_eq!(c.pairs(), [(0, 2)]);
    }

    #[test]
    fn things_are_alive_only_inside_their_span() {
        assert!(alive(None, None, Some(5.0)));
        assert!(alive(Some(1.0), Some(9.0), Some(5.0)));
        assert!(!alive(Some(6.0), None, Some(5.0)));
        assert!(!alive(None, Some(5.0), Some(5.0)), "the end is exclusive");
        assert!(
            alive(Some(6.0), Some(7.0), None),
            "with no time chosen, all of it shows"
        );
    }

    #[test]
    fn weights_stay_above_zero() {
        assert!(Node::new(1, "a", 0).weight(0.0).weight > 0.0);
        assert!(Edge::new(1, 2, "a").weight(-3.0).weight > 0.0);
    }
}
