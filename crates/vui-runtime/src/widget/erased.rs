//! Widgets behind one object-safe trait, so a tree can hold any of them, and the wrapper that maps
//! a component's messages into its parent's.

use super::*;

/// A widget with its state type erased, so nodes can hold any kind.
pub(crate) trait Erased<M: 'static>: 'static {
    fn as_any(&self) -> &dyn Any;
    fn widget_type(&self) -> TypeId;
    /// The widget's type name, without its path or generics: for the inspector.
    fn widget_name(&self) -> &'static str;
    fn create_state(&self) -> Box<dyn Any>;
    fn update(&self, old: &dyn Erased<M>, state: &mut dyn Any) -> Dirty;
    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size;
    fn baseline(&self, state: &dyn Any) -> Option<f32>;
    fn paint(&self, state: &dyn Any, cx: &mut PaintCx);
    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx);
    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool;
    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool;
    fn animating(&self, state: &dyn Any) -> bool;
    fn access(&self, state: &dyn Any, info: &mut AccessInfo);
    fn focusable(&self) -> Focus;
    fn cursor(&self) -> Option<CursorIcon>;
}

pub(crate) fn state_ref<S: 'static>(state: &dyn Any) -> &S {
    state.downcast_ref().expect("a node keeps the state its widget created")
}

pub(crate) fn state_mut<S: 'static>(state: &mut dyn Any) -> &mut S {
    state.downcast_mut().expect("a node keeps the state its widget created")
}

/// A widget as the runtime holds it.
pub(crate) struct Plain<W>(pub W);

impl<M: 'static, W: Widget<M>> Erased<M> for Plain<W> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn widget_type(&self) -> TypeId {
        TypeId::of::<W>()
    }

    fn widget_name(&self) -> &'static str {
        let full = std::any::type_name::<W>();
        let bare = full.split('<').next().unwrap_or(full);
        bare.rsplit("::").next().unwrap_or(bare)
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(Widget::<M>::create_state(&self.0))
    }

    fn update(&self, old: &dyn Erased<M>, state: &mut dyn Any) -> Dirty {
        match old.as_any().downcast_ref::<Plain<W>>() {
            Some(old) => Widget::<M>::update(&self.0, &old.0, state_mut(state)),
            None => Dirty::Layout,
        }
    }

    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        Widget::<M>::measure(&self.0, state_mut(state), env, known, avail)
    }

    fn baseline(&self, state: &dyn Any) -> Option<f32> {
        Widget::<M>::baseline(&self.0, state_ref(state))
    }

    fn paint(&self, state: &dyn Any, cx: &mut PaintCx) {
        Widget::<M>::paint(&self.0, state_ref(state), cx)
    }

    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx) {
        Widget::<M>::paint_over(&self.0, state_ref(state), cx)
    }

    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        Widget::<M>::event(&self.0, state_mut(state), cx, event)
    }

    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool {
        Widget::<M>::tick(&self.0, state_mut(state), cx, dt)
    }

    fn animating(&self, state: &dyn Any) -> bool {
        Widget::<M>::animating(&self.0, state_ref(state))
    }

    fn access(&self, state: &dyn Any, info: &mut AccessInfo) {
        Widget::<M>::access(&self.0, state_ref(state), info)
    }

    fn focusable(&self) -> Focus {
        Widget::<M>::focusable(&self.0)
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Widget::<M>::cursor(&self.0)
    }
}

/// A widget whose messages are turned into another type: what [`Element::map`](crate::Element::map)
/// wraps each widget in, so a component written for its own message type sits inside a bigger app.
pub(crate) struct Mapped<A: 'static, B: 'static> {
    pub inner: Box<dyn Erased<A>>,
    pub f: Rc<dyn Fn(A) -> B>,
}

impl<A: 'static, B: 'static> Erased<B> for Mapped<A, B> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    /// The wrapped widget's type: state is kept across a change of mapping, as it is the same widget.
    fn widget_type(&self) -> TypeId {
        self.inner.widget_type()
    }

    fn widget_name(&self) -> &'static str {
        self.inner.widget_name()
    }

    fn create_state(&self) -> Box<dyn Any> {
        self.inner.create_state()
    }

    fn update(&self, old: &dyn Erased<B>, state: &mut dyn Any) -> Dirty {
        match old.as_any().downcast_ref::<Mapped<A, B>>() {
            Some(old) => self.inner.update(&*old.inner, state),
            None => Dirty::Layout,
        }
    }

    fn measure(&self, state: &mut dyn Any, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        self.inner.measure(state, env, known, avail)
    }

    fn baseline(&self, state: &dyn Any) -> Option<f32> {
        self.inner.baseline(state)
    }

    fn paint(&self, state: &dyn Any, cx: &mut PaintCx) {
        self.inner.paint(state, cx)
    }

    fn paint_over(&self, state: &dyn Any, cx: &mut PaintCx) {
        self.inner.paint_over(state, cx)
    }

    fn event(&self, state: &mut dyn Any, cx: &mut EventCx<B>, event: &WidgetEvent) -> bool {
        // The inner widget writes into an output of its own type; requests and effects pass through
        // unchanged and its messages come back mapped.
        let mut out = Output::<A> {
            messages: Vec::new(),
            effects: std::mem::take(&mut cx.out.effects),
            req: std::mem::take(&mut cx.out.req),
        };
        let used = {
            let mut inner = EventCx {
                env: cx.env,
                id: cx.id,
                rect: cx.rect,
                content: cx.content,
                hovered: cx.hovered,
                pressed: cx.pressed,
                focused: cx.focused,
                focus_visible: cx.focus_visible,
                window_focused: cx.window_focused,
                mods: cx.mods,
                time: cx.time,
                out: &mut out,
                clipboard: cx.clipboard,
            };
            self.inner.event(state, &mut inner, event)
        };
        cx.out.effects = out.effects;
        cx.out.req = out.req;
        cx.out.messages.extend(out.messages.into_iter().map(|m| (self.f)(m)));
        used
    }

    fn tick(&self, state: &mut dyn Any, cx: &mut TickCx, dt: f32) -> bool {
        self.inner.tick(state, cx, dt)
    }

    fn animating(&self, state: &dyn Any) -> bool {
        self.inner.animating(state)
    }

    fn access(&self, state: &dyn Any, info: &mut AccessInfo) {
        self.inner.access(state, info)
    }

    fn focusable(&self) -> Focus {
        self.inner.focusable()
    }

    fn cursor(&self) -> Option<CursorIcon> {
        self.inner.cursor()
    }
}
