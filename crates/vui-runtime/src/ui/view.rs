//! Taking the app's new element tree: matching it against the old, moving focus into what
//! appeared, and forgetting what left.

use super::*;

impl<M: 'static> Ui<M> {
    /// Installs the tree the app wants on screen. Nodes that match keep their state.
    pub fn set_view(&mut self, view: impl IntoElement<M>) {
        let el = view.into_element();
        self.set_view_inner(el);
        self.focus_arrivals();
    }

    /// Focus moves to an element marked `autofocus` that has just appeared, or into a focus trap
    /// that has just appeared and holds no focus yet.
    pub(crate) fn focus_arrivals(&mut self) {
        let Some(root) = self.root.as_ref() else { return };
        let mut showing = Vec::new();
        find_flagged(root, &mut showing);
        let before = std::mem::replace(&mut self.flagged, showing.iter().map(|(id, _)| *id).collect());
        let arrived = showing.into_iter().filter(|(id, _)| !before.contains(id));
        let (mut auto, mut trap) = (None, None);
        for (id, autofocus) in arrived {
            if autofocus {
                auto = auto.or(Some(id));
            } else {
                trap = Some(id);
            }
        }
        // Autofocus on a container lands on the first thing inside it that takes focus.
        let auto = auto.and_then(|id| {
            let node = find_id(root, id)?;
            if node.focus_mode() != Focus::Never { Some(id) } else { collect_stops(node).first().map(|s| s.entry) }
        });
        let target = auto.or_else(|| {
            let trap = trap?;
            let node = find_id(root, trap)?;
            if self.core.focus.is_some_and(|f| find_id(node, f).is_some()) {
                return None;
            }
            collect_stops(node).first().map(|s| s.entry)
        });
        if let Some(id) = target {
            self.sync();
            let keyboard = self.core.keyboard;
            if let Some(root) = self.root.as_mut() {
                Run { root, core: &mut self.core }.set_focus(Some(id), keyboard);
            }
        }
    }

    pub(crate) fn set_view_inner(&mut self, el: crate::element::Element<M>) {
        let core = &mut self.core;
        let dirty = match self.root.as_mut() {
            Some(root) if root.widget.widget_type() == el.widget.widget_type() => root.update(el, &mut core.ids, &core.env),
            _ => {
                self.root = Some(Node::build(el, &mut core.ids, &core.env));
                crate::widget::Dirty::Layout
            }
        };
        match dirty {
            crate::widget::Dirty::Layout => core.layout_dirty = true,
            crate::widget::Dirty::Paint => core.paint_dirty = true,
            crate::widget::Dirty::None => {}
        }
        // A transition or a widget animation begun by this update needs frames from here on.
        core.animating |= self.root.as_ref().is_some_and(Node::animating);
        self.forget_missing();
    }

    /// Drops every reference to nodes that are no longer in the tree.
    pub(crate) fn forget_missing(&mut self) {
        let core = &mut self.core;
        let Some(root) = self.root.as_ref() else { return };
        let mut live = HashSet::new();
        collect_ids(root, &mut live);
        if core.focus.is_some_and(|f| !live.contains(&f)) {
            core.focus = None;
            core.focus_visible = false;
            core.out.effects.push(Effect::ImeArea(None));
        }
        core.hover.retain(|id| live.contains(id));
        if core.press.as_ref().is_some_and(|p| !live.contains(&p.id)) {
            core.press = None;
        }
        if core.bar_drag.as_ref().is_some_and(|d| !live.contains(&d.id)) {
            core.bar_drag = None;
        }
        if core.drop_target.is_some_and(|d| !live.contains(&d)) {
            core.drop_target = None;
        }
    }
}

/// The ids of the nodes in the view: not ones fading out after the view dropped them.
pub(crate) fn collect_ids<M: 'static>(node: &Node<M>, out: &mut HashSet<NodeId>) {
    if node.leaving {
        return;
    }
    out.insert(node.id);
    for child in &node.children {
        collect_ids(child, out);
    }
}

pub(crate) fn clear_caches<M: 'static>(node: &mut Node<M>) {
    node.cache.clear();
    for child in &mut node.children {
        clear_caches(child);
    }
}
