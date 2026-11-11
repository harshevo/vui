//! The graph as a widget. It lays the graph out and keeps it moving until it settles, then lets you look
//! around: drag to pan or to pull a node, scroll or pinch to zoom, click to pick, double-click a node to
//! open it or empty space to fit everything again.
//!
//! The app owns what is picked and what is lit; the widget reports clicks and draws what it is given.

use std::cell::Cell;
use std::rc::Rc;

use vui_core::{CursorIcon, Event, Key, KeyEvent, NamedKey, Point, PointerButton, Size, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Role, TickCx, Widget,
    WidgetEvent, builder,
};

use super::camera::Camera;
use super::data::{Compiled, GraphData, alive};
use super::layout::Layout;
use super::render::{Frame, Lit, draw};
use crate::canvas::{Canvas, paint};
use crate::style::VizStyle;

mod builder;
mod state;

pub use builder::{GraphView, graph_view};
pub use state::GraphState;
use state::*;

const MARGIN: f32 = 40.0;
/// How far the pointer may move and still count as a click.
const SLOP: f32 = 4.0;
const GROW_SECONDS: f32 = 0.45;

/// What to light up, by node id. Edges are named by the ids of their two ends. With only `nodes`, the edges
/// between them light up too.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Highlight {
    pub nodes: Vec<u64>,
    pub edges: Vec<(u64, u64)>,
    /// Nodes that also send out rings: where an answer is, or where to look.
    pub pulse: Vec<u64>,
}

type Select<M> = Rc<dyn Fn(Option<u64>) -> M>;
type Open<M> = Rc<dyn Fn(u64) -> M>;

pub struct GraphWidget<M> {
    data: Rc<GraphData>,
    selected: Option<u64>,
    highlight: Highlight,
    as_of: Option<f64>,
    label: String,
    on_select: Option<Select<M>>,
    on_open: Option<Open<M>>,
}

impl<M> GraphWidget<M> {
    fn style(env: &Env) -> VizStyle {
        VizStyle::from_palette(env.palette())
    }

    fn selected_index(&self, state: &GraphState) -> Option<usize> {
        self.selected
            .and_then(|id| state.graph.index.get(&id).copied())
    }

    fn fit_target(&self, state: &GraphState, area: vui_core::Rect) -> Camera {
        let live = (0..self.data.nodes.len()).filter(|&i| {
            let n = &self.data.nodes[i];
            alive(n.since, n.until, self.as_of)
        });
        Camera::fit(live.map(|i| state.layout.position(i)), area, MARGIN)
    }

    /// The frame as it stands, for finding what is under the pointer.
    fn with_frame<R>(
        &self,
        state: &GraphState,
        env: &Env,
        area: vui_core::Rect,
        f: impl FnOnce(&Frame) -> R,
    ) -> R {
        let pos: Vec<Point> = state.layout.positions().collect();
        let style = Self::style(env);
        let frame = Frame {
            data: &self.data,
            graph: &state.graph,
            pos: &pos,
            camera: state.shown.get().unwrap_or(state.goal),
            area,
            hover: state.hover,
            selected: self.selected_index(state),
            lit: &state.lit,
            as_of: self.as_of,
            grown: &state.grown,
            clock: state.clock,
            style: &style,
        };
        f(&frame)
    }
}

impl<M: 'static> Widget<M> for GraphWidget<M> {
    type State = GraphState;

    fn create_state(&self) -> GraphState {
        GraphState::new(&self.data, &self.highlight)
    }

    fn update(&self, old: &GraphWidget<M>, state: &mut GraphState) -> Dirty {
        if !Rc::ptr_eq(&self.data, &old.data) && *self.data != *old.data {
            let graph = Compiled::new(&self.data);
            // Nodes that were there keep their place and their age; the others grow in.
            let keep: Vec<Option<usize>> = self
                .data
                .nodes
                .iter()
                .map(|n| state.graph.index.get(&n.id).copied())
                .collect();
            let grown: Vec<f32> = keep
                .iter()
                .map(|k| k.and_then(|o| state.grown.get(o).copied()).unwrap_or(0.0))
                .collect();
            state
                .layout
                .rebuild(self.data.nodes.len(), &graph.pairs(), &keep);
            state.hover = state
                .hover
                .and_then(|h| old.data.nodes.get(h))
                .and_then(|n| graph.index.get(&n.id).copied());
            state.drag = None;
            state.graph = graph;
            state.grown = grown;
            state.lit = resolve(&state.graph, &self.highlight);
        } else if self.highlight != old.highlight {
            state.lit = resolve(&state.graph, &self.highlight);
        }
        Dirty::Paint
    }

    fn measure(
        &self,
        _state: &mut GraphState,
        _env: &Env,
        known: Pair<Option<f32>>,
        avail: Pair<Avail>,
    ) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            _ => 640.0,
        });
        let height = known.y.unwrap_or(match avail.y {
            Avail::Definite(h) => h,
            _ => (width * 0.65).clamp(240.0, 640.0),
        });
        size(width, height)
    }

    fn paint(&self, state: &GraphState, cx: &mut PaintCx) {
        let area = cx.content;
        let target = if state.follow {
            self.fit_target(state, area)
        } else {
            state.goal
        };
        let dt = (cx.time - state.last_paint.get()).clamp(0.0, 0.1);
        state.last_paint.set(cx.time);
        let next = match state.shown.get() {
            None => target,
            Some(cur) => cur.toward(target, 1.0 - (-dt * 9.0).exp()),
        };
        state.shown.set(Some(next));
        state.easing.set(!next.is_near(target));

        let pos: Vec<Point> = state.layout.positions().collect();
        let style = Self::style(cx.env);
        let frame = Frame {
            data: &self.data,
            graph: &state.graph,
            pos: &pos,
            camera: next,
            area,
            hover: state.hover,
            selected: self.selected_index(state),
            lit: &state.lit,
            as_of: self.as_of,
            grown: &state.grown,
            clock: state.clock,
            style: &style,
        };
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        draw(&mut canvas, &frame);
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
    }

    fn event(&self, state: &mut GraphState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let area = cx.content;
        let mid = area.center();
        match event {
            WidgetEvent::Input(Event::PointerDown {
                pos,
                button: PointerButton::Primary,
                click_count,
                ..
            }) => {
                let hit = self.with_frame(state, cx.env, area, |f| f.hit(*pos));
                if *click_count >= 2 {
                    match hit {
                        Some(i) => {
                            if let Some(f) = &self.on_open {
                                cx.emit(f(self.data.nodes[i].id));
                            }
                        }
                        None => {
                            state.follow = true;
                            cx.request_frame();
                        }
                    }
                } else {
                    state.drag = Some(match hit {
                        Some(index) => Drag::Node {
                            index,
                            start: *pos,
                            moved: false,
                        },
                        None => Drag::Pan {
                            last: *pos,
                            start: *pos,
                            moved: false,
                        },
                    });
                    cx.capture_pointer();
                }
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                state.pointer = Some(*pos);
                let camera = state.shown.get().unwrap_or(state.goal);
                match &mut state.drag {
                    Some(Drag::Pan { last, start, moved }) => {
                        *moved |= start.distance(*pos) > SLOP;
                        if *moved {
                            let (dx, dy) = (pos.x - last.x, pos.y - last.y);
                            *last = *pos;
                            state.take_camera();
                            state.goal.pan(dx, dy);
                            state.snap();
                        }
                        cx.request_frame();
                    }
                    Some(Drag::Node {
                        index,
                        start,
                        moved,
                    }) => {
                        *moved |= start.distance(*pos) > SLOP;
                        if *moved {
                            state.layout.hold(*index, camera.to_world(*pos, mid));
                            state.layout.keep_warm(0.3);
                        }
                        cx.request_frame();
                    }
                    None => {
                        let hit = self.with_frame(state, cx.env, area, |f| f.hit(*pos));
                        if hit != state.hover {
                            state.hover = hit;
                        }
                    }
                }
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => {
                let Some(drag) = state.drag.take() else {
                    return false;
                };
                cx.release_pointer();
                match drag {
                    Drag::Node {
                        index, moved: true, ..
                    } => {
                        state.layout.release(index);
                        state.layout.keep_warm(0.0);
                    }
                    Drag::Node {
                        index,
                        moved: false,
                        ..
                    } => self.select(cx, Some(self.data.nodes[index].id)),
                    Drag::Pan { moved: false, .. } if self.selected.is_some() => {
                        self.select(cx, None)
                    }
                    Drag::Pan { .. } => {}
                }
                cx.request_frame();
                true
            }
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => {
                state.pointer = None;
                state.hover = None;
                cx.request_paint();
                false
            }
            WidgetEvent::Input(Event::Scroll {
                pos,
                delta,
                precise,
                ..
            }) => {
                state.take_camera();
                if *precise && !(cx.mods.cmd || cx.mods.ctrl) {
                    state.goal.pan(delta.x, delta.y);
                    state.snap();
                } else {
                    let factor = (delta.y * 0.004).exp().clamp(0.5, 2.0);
                    state.goal.zoom_about(*pos, mid, factor);
                    if *precise {
                        state.snap();
                    }
                }
                cx.request_frame();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Magnify { pos, delta, .. }) => {
                state.take_camera();
                state.goal.zoom_about(*pos, mid, (1.0 + delta).max(0.2));
                state.snap();
                cx.request_paint();
                true
            }
            WidgetEvent::Input(Event::Key(key)) => self.key(state, cx, key),
            _ => false,
        }
    }

    fn tick(&self, state: &mut GraphState, _cx: &mut TickCx, dt: f32) -> bool {
        state.clock += dt;
        let n = state.layout.len().max(1);
        for _ in 0..(4000 / n).clamp(1, 3) {
            state.layout.step();
        }
        let mut growing = false;
        for g in &mut state.grown {
            if *g < 1.0 {
                *g = (*g + dt / GROW_SECONDS).min(1.0);
                growing = true;
            }
        }
        !state.layout.is_settled() || growing || !state.lit.pulse.is_empty() || state.easing.get()
    }

    fn animating(&self, state: &GraphState) -> bool {
        !state.layout.is_settled()
            || state.grown.iter().any(|g| *g < 1.0)
            || !state.lit.pulse.is_empty()
            || state.easing.get()
    }

    fn access(&self, _state: &GraphState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        info.label = Some(if self.label.is_empty() {
            format!(
                "Graph of {} nodes and {} links",
                self.data.nodes.len(),
                self.data.edges.len()
            )
        } else {
            self.label.clone()
        });
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Grab)
    }
}

impl<M: 'static> GraphWidget<M> {
    fn select(&self, cx: &mut EventCx<M>, id: Option<u64>) {
        if let Some(f) = &self.on_select {
            cx.emit(f(id));
        }
    }

    fn key(&self, state: &mut GraphState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed || key.mods.cmd || key.mods.ctrl || key.mods.alt {
            return false;
        }
        let mid = cx.content.center();
        match key.key {
            Key::Character('+' | '=') => {
                state.take_camera();
                state.goal.zoom_about(mid, mid, 1.4);
            }
            Key::Character('-') => {
                state.take_camera();
                state.goal.zoom_about(mid, mid, 1.0 / 1.4);
            }
            Key::Character('0') => state.follow = true,
            Key::Named(NamedKey::Escape) => self.select(cx, None),
            Key::Named(
                arrow @ (NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown),
            ) => {
                state.take_camera();
                let (dx, dy) = match arrow {
                    NamedKey::ArrowLeft => (60.0, 0.0),
                    NamedKey::ArrowRight => (-60.0, 0.0),
                    NamedKey::ArrowUp => (0.0, 60.0),
                    _ => (0.0, -60.0),
                };
                state.goal.pan(dx, dy);
            }
            _ => return false,
        }
        cx.request_frame();
        cx.request_paint();
        true
    }
}

builder!(GraphView<M> => GraphWidget<M>);

#[cfg(test)]
mod tests;
