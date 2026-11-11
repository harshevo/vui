//! Building nodes from elements and updating them from the next tree: keeping state, retargeting
//! springs, and saying how much work the change needs.

use super::*;

impl<M: 'static> Node<M> {
    pub fn build(el: Element<M>, ids: &mut Ids, env: &Env) -> Node<M> {
        let Element { base, widget, children } = el;
        let Base { style: asked, rules, decor, input, a11y, key, transition, appear } = base;
        let style = responsive::resolve(asked, &rules, env.window);
        let opacity = decor.opacity;
        let state = widget.create_state();
        let mut anim = Anim::new(opacity);
        if appear {
            anim.appear(opacity, transition.unwrap_or(Spring::SMOOTH));
        }
        Node {
            id: ids.next(),
            key,
            widget,
            state,
            asked,
            rules,
            target: style,
            eff: style,
            decor,
            input,
            a11y,
            transition,
            children: children.into_iter().map(|c| Node::build(c, ids, env)).collect(),
            cache: LayoutCache::default(),
            layout: NodeLayout::default(),
            rect: Rect::ZERO,
            clip: None,
            scroll: Scroll::new(),
            anim,
            memory: None,
        }
    }

    /// Brings the node in line with a new element for the same place in the tree and says how much
    /// has to be redone.
    pub fn update(&mut self, el: Element<M>, ids: &mut Ids, env: &Env) -> Dirty {
        let Element { base, widget, children } = el;
        let Base { style: asked, rules, decor, input, a11y, key: _, transition, appear: _ } = base;

        let mut dirty = widget.update(&*self.widget, &mut *self.state);
        self.widget = widget;
        self.transition = transition;

        self.asked = asked;
        self.rules = rules;
        if self.retarget_style(env.window) {
            dirty = dirty.max(Dirty::Layout);
        }

        if decor != self.decor {
            self.retarget_decor(&decor, env);
            self.decor = decor;
            dirty = dirty.max(Dirty::Paint);
        }
        if input.disabled != self.input.disabled {
            dirty = dirty.max(Dirty::Paint);
        }
        self.input = input;
        self.a11y = a11y;

        dirty = dirty.max(reconcile(&mut self.children, children, ids, env));
        if dirty == Dirty::Layout {
            self.cache.clear();
        }
        dirty
    }

    /// Resolves the element's style for a window `width` wide and moves towards it, animating sizes
    /// that have a transition. Says whether what layout reads changed.
    pub(crate) fn retarget_style(&mut self, width: f32) -> bool {
        let style = responsive::resolve(self.asked, &self.rules, width);
        let transition = self.transition;
        if style != self.target {
            self.anim.width = retarget(self.anim.width.take(), self.target.width, style.width, transition);
            self.anim.height = retarget(self.anim.height.take(), self.target.height, style.height, transition);
            let (old, new) = (sides(&self.target.inset), sides(&style.inset));
            for (i, slot) in self.anim.inset.iter_mut().enumerate() {
                *slot = retarget(slot.take(), old[i], new[i], transition);
            }
            self.target = style;
        }
        let before = self.eff;
        self.refresh_eff();
        self.eff != before
    }

    /// The window is now `width` wide: re-resolves every responsive rule in the subtree and clears
    /// the layout of whatever changed and of its ancestors. Says whether anything changed.
    pub fn respond(&mut self, width: f32) -> bool {
        let mut changed = false;
        for child in &mut self.children {
            changed |= child.respond(width);
        }
        if !self.rules.is_empty() {
            changed |= self.retarget_style(width);
        }
        if changed {
            self.cache.clear();
        }
        changed
    }

    pub(crate) fn refresh_eff(&mut self) {
        self.eff = self.target;
        if let Some(a) = &self.anim.width {
            self.eff.width = Len::Px(a.value());
        }
        if let Some(a) = &self.anim.height {
            self.eff.height = Len::Px(a.value());
        }
        let inset = &mut self.eff.inset;
        for (slot, a) in [&mut inset.top, &mut inset.right, &mut inset.bottom, &mut inset.left].into_iter().zip(&self.anim.inset) {
            if let Some(a) = a {
                *slot = Len::Px(a.value());
            }
        }
    }

    pub(crate) fn retarget_decor(&mut self, new: &Decor, env: &Env) {
        if new.fill != self.decor.fill {
            self.anim.fill = self.transition.map(|spring| {
                let mut t = SpringValue::new(0.0, spring);
                t.set_target(1.0);
                FillAnim { from: self.base_fill(&env.theme.palette), t }
            });
        }
        if new.opacity != self.anim.opacity.target() {
            match self.transition {
                Some(spring) => {
                    self.anim.opacity.spring = spring;
                    self.anim.opacity.set_target(new.opacity);
                }
                None => self.anim.opacity.snap_to(new.opacity),
            }
        }
    }
}

/// Matches new child elements to the nodes already there: by key when they have one, else by position
/// and widget type. Matches keep their state; the rest are built fresh, and nodes nobody claimed are
/// dropped.
pub(crate) fn reconcile<M: 'static>(old: &mut Vec<Node<M>>, new: Vec<Element<M>>, ids: &mut Ids, env: &Env) -> Dirty {
    let before: Vec<NodeId> = old.iter().map(|n| n.id).collect();
    let mut pool: Vec<Option<Node<M>>> = std::mem::take(old).into_iter().map(Some).collect();
    let by_key: HashMap<Key, usize> =
        pool.iter().enumerate().filter_map(|(i, n)| n.as_ref().and_then(|n| n.key.clone()).map(|k| (k, i))).collect();

    let mut dirty = Dirty::None;
    let mut out: Vec<Node<M>> = Vec::with_capacity(new.len());
    for (i, el) in new.into_iter().enumerate() {
        let kind = el.widget.widget_type();
        let same_kind = |slot: &Option<Node<M>>| slot.as_ref().is_some_and(|n| n.widget.widget_type() == kind);
        let found = match &el.base.key {
            Some(key) => by_key.get(key).copied().filter(|&j| same_kind(&pool[j])),
            None => (i < pool.len() && same_kind(&pool[i]) && pool[i].as_ref().is_some_and(|n| n.key.is_none())).then_some(i),
        };
        match found.and_then(|j| pool[j].take()) {
            Some(mut node) => {
                dirty = dirty.max(node.update(el, ids, env));
                out.push(node);
            }
            None => {
                out.push(Node::build(el, ids, env));
                dirty = Dirty::Layout;
            }
        }
    }
    if pool.iter().any(Option::is_some) || out.iter().map(|n| n.id).ne(before.iter().copied()) {
        dirty = Dirty::Layout;
    }
    *old = out;
    dirty
}
