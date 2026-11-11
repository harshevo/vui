//! Walks over the retained tree: hit testing, the Tab order and focus groups, layers that dismiss
//! and trap focus, fixed placement, and handing an event to one widget.

use super::*;

/// Lays out every fixed element against the window, outer ones first.
pub(crate) fn lay_out_fixed<M: 'static>(node: &mut Node<M>, env: &mut Env, window: Size) {
    for child in &mut node.children {
        if child.eff.hidden {
            continue;
        }
        if child.eff.position == Position::Fixed {
            place_fixed(child, env, window);
            // Kept inside the window: a menu opened near the right edge opens leftwards.
            let (at, size) = (child.layout.origin, child.layout.size);
            child.layout.origin = vui_core::pt(at.x.min(window.w - size.w).max(0.0), at.y.min(window.h - size.h).max(0.0));
        }
        lay_out_fixed(child, env, window);
    }
}

/// The showing elements marked `autofocus` (true) or `trap_focus` (false), in tree order.
pub(crate) fn find_flagged<M: 'static>(node: &Node<M>, out: &mut Vec<(NodeId, bool)>) {
    if node.eff.hidden {
        return;
    }
    if node.input.autofocus {
        out.push((node.id, true));
    } else if node.input.trap_focus {
        out.push((node.id, false));
    }
    for child in &node.children {
        find_flagged(child, out);
    }
}

/// Paths to the elements that close on an outside press or Escape (`on_dismiss`), in the order they
/// are drawn: the last is on top.
pub(crate) fn dismissible<M: 'static>(root: &Node<M>) -> Vec<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, depth: usize, path: &mut Vec<usize>, out: &mut Vec<(usize, Vec<usize>)>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            let depth = depth + usize::from(child.eff.float);
            if child.input.dismiss.is_some() {
                out.push((depth, path.clone()));
            }
            walk(child, depth, path, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut Vec::new(), &mut out);
    out.sort_by_key(|(depth, _)| *depth);
    out.into_iter().map(|(_, p)| p).collect()
}

/// The innermost focus trap that shows, if any: Tab stays inside it.
pub(crate) fn focus_trap<M: 'static>(root: &Node<M>) -> Option<Vec<usize>> {
    fn walk<M: 'static>(node: &Node<M>, path: &mut Vec<usize>, best: &mut Option<Vec<usize>>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            if child.input.trap_focus {
                *best = Some(path.clone());
            }
            walk(child, path, best);
            path.pop();
        }
    }
    let mut best = None;
    walk(root, &mut Vec::new(), &mut best);
    best
}

/// The child indices down to the topmost node that takes pointer input at `p`.
pub(crate) fn hit<M: 'static>(root: &Node<M>, p: Point) -> Option<Vec<usize>> {
    /// Paths to the floating elements, each with how many floats it is inside of.
    fn floats<M: 'static>(node: &Node<M>, depth: usize, path: &mut Vec<usize>, out: &mut Vec<(usize, Vec<usize>)>) {
        for (i, child) in node.children.iter().enumerate() {
            if child.eff.hidden {
                continue;
            }
            path.push(i);
            if child.eff.float {
                out.push((depth, path.clone()));
            }
            floats(child, depth + usize::from(child.eff.float), path, out);
            path.pop();
        }
    }
    fn walk<M: 'static>(node: &Node<M>, p: Point, path: &mut Vec<usize>) -> bool {
        if node.eff.hidden || node.clip.is_some_and(|c| !c.contains(p)) {
            return false;
        }
        if node.eff.overflow != Overflow::Visible && !node.rect.contains(p) {
            return false;
        }
        for (i, child) in node.children.iter().enumerate().rev() {
            if child.eff.float {
                continue;
            }
            path.push(i);
            if walk(child, p, path) {
                return true;
            }
            path.pop();
        }
        node.input.pointer_events && node.rect.contains(p)
    }
    let mut above = Vec::new();
    floats(root, 0, &mut Vec::new(), &mut above);
    // Floats paint in tree order, and the floats inside them after all of those: the reverse of
    // that is the order they are hit in.
    above.sort_by_key(|(depth, _)| *depth);
    for (_, mut path) in above.into_iter().rev() {
        let node = path.iter().fold(root, |n, &i| &n.children[i]);
        if walk(node, p, &mut path) {
            return Some(path);
        }
    }
    let mut path = Vec::new();
    walk(root, p, &mut path).then_some(path)
}

/// One place Tab stops: a lone focusable element, or a whole focus group.
pub(crate) struct Stop {
    /// Where Tab lands.
    pub(crate) entry: NodeId,
    /// 0 for the first item, 1 for the item that had focus last, 2 for the item marked `tab_default`.
    rank: u8,
    /// Everything that counts as being on this stop.
    pub(crate) members: Vec<NodeId>,
}

/// The Tab order. Every focusable element is a stop of its own, except the items of a focus group,
/// which share one stop where the group's first item is.
pub(crate) fn collect_stops<M: 'static>(root: &Node<M>) -> Vec<Stop> {
    struct Walk {
        stops: Vec<Stop>,
        /// Group node to its stop.
        groups: HashMap<NodeId, usize>,
    }
    /// `group` is the nearest group above `node` and the item it last had focus on.
    fn walk<M: 'static>(node: &Node<M>, group: Option<(NodeId, Option<NodeId>)>, w: &mut Walk) {
        if node.eff.hidden || node.input.disabled {
            return;
        }
        if node.focus_mode() != Focus::Never {
            match group {
                None => w.stops.push(Stop { entry: node.id, rank: 0, members: vec![node.id] }),
                Some((g, memory)) => {
                    let i = match w.groups.get(&g) {
                        Some(&i) => i,
                        None => {
                            w.stops.push(Stop { entry: node.id, rank: 0, members: Vec::new() });
                            w.groups.insert(g, w.stops.len() - 1);
                            w.stops.len() - 1
                        }
                    };
                    let rank = if node.input.tab_default {
                        2
                    } else if memory == Some(node.id) {
                        1
                    } else {
                        0
                    };
                    let stop = &mut w.stops[i];
                    stop.members.push(node.id);
                    if rank > stop.rank {
                        stop.entry = node.id;
                        stop.rank = rank;
                    }
                }
            }
        }
        // A group's own focus belongs to the context around it; what is inside belongs to the group.
        let inner = if node.input.group.is_some() { Some((node.id, node.memory)) } else { group };
        for child in &node.children {
            walk(child, inner, w);
        }
    }
    let mut w = Walk { stops: Vec::new(), groups: HashMap::new() };
    walk(root, None, &mut w);
    w.stops
}

/// The focusable elements that belong to this focus group, in order: those inside it that are not
/// inside a group of their own.
pub(crate) fn group_items<M: 'static>(group: &Node<M>) -> Vec<NodeId> {
    fn walk<M: 'static>(node: &Node<M>, out: &mut Vec<NodeId>) {
        if node.eff.hidden || node.input.disabled {
            return;
        }
        if node.focus_mode() != Focus::Never {
            out.push(node.id);
        }
        if node.input.group.is_none() {
            for child in &node.children {
                walk(child, out);
            }
        }
    }
    let mut out = Vec::new();
    for child in &group.children {
        walk(child, &mut out);
    }
    out
}

/// Hands one event to one widget and returns whether it used it.
pub(crate) fn call_widget<M: 'static>(core: &mut Core<M>, node: &mut Node<M>, event: &WidgetEvent) -> bool {
    let focused = core.focus == Some(node.id);
    let mut cx = EventCx {
        env: &core.env,
        id: node.id,
        rect: node.rect,
        content: node.content_rect(node.rect),
        hovered: core.hover.contains(&node.id),
        pressed: core.press.as_ref().is_some_and(|p| p.id == node.id && p.inside),
        focused,
        focus_visible: focused && core.focus_visible,
        window_focused: core.window_focused,
        mods: core.mods,
        time: core.time,
        out: &mut core.out,
        clipboard: &*core.clipboard,
    };
    node.widget.event(&mut *node.state, &mut cx, event)
}

/// The tree and the core together, so event handling can reach both at once.
pub(crate) struct Run<'a, M: 'static> {
    pub root: &'a mut Node<M>,
    pub core: &'a mut Core<M>,
}

/// The pointer shape a chain of nodes asks for, innermost first. A disabled node stops the search.
pub(crate) fn cursor_of<M: 'static>(nodes: &[&Node<M>]) -> Option<CursorIcon> {
    for node in nodes.iter().rev() {
        if node.input.disabled {
            return Some(CursorIcon::Arrow);
        }
        if let Some(c) = node.input.cursor.or_else(|| node.widget.cursor()) {
            return Some(c);
        }
    }
    None
}
