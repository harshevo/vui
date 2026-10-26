//! Building the accessibility tree from the node tree, and carrying out what assistive technology asks.

use vui_core::Rect;

use crate::access::{AccessAction, AccessActionKind, AccessInfo, AccessNode, AccessTree, Role};
use crate::key::NodeId;
use crate::node::{Node, chain, node_at, path_of};
use crate::ui::{Run, Ui};
use crate::widget::{Focus, WidgetEvent};

/// Elements that are one thing to assistive technology: whatever is inside them is folded into their
/// name rather than listed separately.
fn is_atomic(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::Checkbox
            | Role::Switch
            | Role::RadioButton
            | Role::Slider
            | Role::TextInput
            | Role::SearchInput
            | Role::Tab
            | Role::MenuItem
            | Role::ProgressBar
            | Role::Image
            | Role::Label
            | Role::Heading
    )
}

fn info_of<M: 'static>(node: &Node<M>) -> AccessInfo {
    let mut info = AccessInfo::default();
    node.widget.access(&*node.state, &mut info);
    info
}

/// The visible text inside a node, for naming a button after its label.
fn text_inside<M: 'static>(node: &Node<M>, out: &mut Vec<String>) {
    if node.eff.hidden || node.a11y.hidden {
        return;
    }
    let info = info_of(node);
    if matches!(info.role, Some(Role::Label | Role::Heading))
        && let Some(text) = info.label
    {
        out.push(text);
    }
    for child in &node.children {
        text_inside(child, out);
    }
}

/// Adds the node, or if it is layout only, whatever is inside it. Returns the ids the parent should
/// list as children. `inert` is true inside a disabled element: everything in there is disabled too.
fn build<M: 'static>(node: &Node<M>, inert: bool, out: &mut Vec<AccessNode>) -> Vec<u64> {
    if node.eff.hidden || node.a11y.hidden {
        return Vec::new();
    }
    let inert = inert || node.input.disabled;
    let info = info_of(node);
    let clickable = node.input.clickable();
    let focus = node.focus_mode();
    let scrolls = node.eff.overflow == crate::style::Overflow::Scroll;
    // A box that can be clicked, scrolled or focused is something to assistive technology even if
    // nobody gave it a role.
    let fallback = if clickable {
        Role::Button
    } else if scrolls {
        Role::ScrollView
    } else if focus != Focus::Never {
        Role::Group
    } else {
        Role::None
    };
    let role = node.a11y.role.or(info.role).unwrap_or(fallback);
    if role == Role::None {
        return node.children.iter().flat_map(|c| build(c, inert, out)).collect();
    }

    let atomic = is_atomic(role);
    let mut name = node.a11y.label.clone().or(info.label.clone());
    if name.is_none() && atomic && !matches!(role, Role::Label | Role::Heading | Role::Image) {
        let mut parts = Vec::new();
        text_inside(node, &mut parts);
        if !parts.is_empty() {
            name = Some(parts.join(" "));
        }
    }

    // A disabled element can be seen but not acted on.
    let mut actions = Vec::new();
    if !inert {
        actions.clone_from(&info.actions);
        if clickable && !actions.contains(&AccessActionKind::Click) {
            actions.push(AccessActionKind::Click);
        }
        if focus != Focus::Never && !actions.contains(&AccessActionKind::Focus) {
            actions.push(AccessActionKind::Focus);
        }
        if scrolls {
            actions.push(AccessActionKind::ScrollIntoView);
        }
    }

    let children = if atomic { Vec::new() } else { node.children.iter().flat_map(|c| build(c, inert, out)).collect() };
    let id = node.id.0 + 1;
    out.push(AccessNode {
        id,
        role,
        name,
        description: node.a11y.description.clone(),
        value: info.value,
        bounds: node.rect,
        children,
        toggled: info.toggled,
        mixed: info.mixed,
        selected: info.selected,
        expanded: info.expanded,
        range: info.range,
        selection: info.selection,
        disabled: inert,
        focusable: focus != Focus::Never && !inert,
        actions,
    });
    vec![id]
}

impl<M: 'static> Ui<M> {
    /// Everything assistive technology can see, laid out and current.
    pub fn access_tree(&mut self) -> AccessTree {
        self.sync();
        let size = self.core.size;
        let mut nodes = Vec::new();
        let children = self.root.as_ref().map(|root| build(root, false, &mut nodes)).unwrap_or_default();
        let focus = self.core.focus.map(|f| f.0 + 1).filter(|id| nodes.iter().any(|n| n.id == *id)).unwrap_or(0);
        nodes.push(AccessNode {
            id: 0,
            role: Role::Window,
            name: None,
            description: None,
            value: None,
            bounds: Rect::new(0.0, 0.0, size.w, size.h),
            children,
            toggled: None,
            mixed: false,
            selected: None,
            expanded: None,
            range: None,
            selection: None,
            disabled: false,
            focusable: false,
            actions: Vec::new(),
        });
        // The window leads: platform adapters expect the root to be named first in a full update.
        nodes.rotate_right(1);
        AccessTree { nodes, focus }
    }

    /// Carries out something assistive technology asked of the node with this id.
    pub fn do_action(&mut self, id: u64, action: AccessAction) {
        self.sync();
        let Some(root) = self.root.as_mut() else { return };
        let Some(raw) = id.checked_sub(1) else { return };
        let Some(path) = path_of(root, NodeId(raw)) else { return };
        // What assistive technology cannot see or is told it cannot use is not acted on, even if a stale
        // id or a careless client asks.
        if chain(root, &path).iter().any(|n| n.input.disabled || n.eff.hidden || n.a11y.hidden) {
            return;
        }
        let mut run = Run { root, core: &mut self.core };
        match action {
            AccessAction::Click => run.activate(&path),
            AccessAction::Focus => {
                if node_at(run.root, &path).focus_mode() != Focus::Never {
                    run.set_focus(Some(NodeId(raw)), true);
                    let rect = node_at(run.root, &path).rect;
                    run.reveal(&path, rect);
                }
            }
            AccessAction::ScrollIntoView => {
                let rect = node_at(run.root, &path).rect;
                run.reveal(&path, rect);
            }
            other => {
                run.call(&path, &WidgetEvent::Action(&other));
            }
        }
        self.pump();
        self.sync();
    }
}
