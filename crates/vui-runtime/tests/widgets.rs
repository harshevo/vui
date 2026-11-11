//! Custom widgets written against the public `Widget` trait: what a widget author gets to use.

mod common;

use common::*;
use vui_core::{CursorIcon, Event, NamedKey, Size, Tone, pt, rect, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::testing::quads;
use vui_runtime::{
    Align, Base, Builder, Dirty, Effect, Element, Env, EventCx, Focus, Overflow, PaintCx, Parent, Styled,
    Widget, WidgetEvent, builder, column, div, row, spacer,
};

// ---- a slider: holds on to the pointer while it is dragged ------------------------------------

struct Slider;

#[derive(Default)]
struct Dragging(bool);

fn value_at(cx: &EventCx<Msg>, x: f32) -> i32 {
    (((x - cx.rect.x) / cx.rect.w).clamp(0.0, 1.0) * 100.0).round() as i32
}

impl Widget<Msg> for Slider {
    type State = Dragging;

    fn create_state(&self) -> Dragging {
        Dragging::default()
    }

    fn event(&self, state: &mut Dragging, cx: &mut EventCx<Msg>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, .. }) => {
                state.0 = true;
                let value = value_at(cx, pos.x);
                cx.emit(Msg::Value(value));
                true
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if state.0 => {
                let value = value_at(cx, pos.x);
                cx.emit(Msg::Value(value));
                true
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => {
                state.0 = false;
                true
            }
            _ => false,
        }
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Grab)
    }
}

struct SliderEl<M: 'static> {
    base: Base<M>,
    widget: Slider,
}
builder!(SliderEl<M> => Slider);

fn slider() -> SliderEl<Msg> {
    SliderEl { base: Base::new(), widget: Slider }
}

#[test]
fn a_dragged_widget_keeps_the_pointer_when_it_leaves_its_bounds() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(row().height(20.0).children((spacer(), slider().key("s").width(100.0)))));
    assert_eq!(t.rect("s").x, 200.0);
    t.press(pt(250.0, 10.0));
    assert_eq!(t.messages(), [Msg::Value(50)]);
    t.move_to(pt(10.0, 500.0));
    assert_eq!(t.messages(), [Msg::Value(0)], "far outside, still following, clamped");
    t.move_to(pt(290.0, 50.0));
    assert_eq!(t.messages(), [Msg::Value(90)]);
    t.release(pt(290.0, 50.0));
    t.move_to(pt(210.0, 10.0));
    assert!(t.messages().is_empty(), "after the release it no longer follows");
}

#[test]
fn a_widget_sets_the_cursor_over_itself() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(slider().height(20.0)));
    t.effects();
    t.move_to(pt(10.0, 10.0));
    assert_eq!(t.effects(), [Effect::Cursor(CursorIcon::Grab)]);
    t.move_to(pt(10.0, 90.0));
    assert_eq!(t.effects(), [Effect::Cursor(CursorIcon::Arrow)]);
}

#[test]
fn an_element_can_override_the_cursor_of_its_widget() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(slider().height(20.0).cursor(CursorIcon::NotAllowed)));
    t.effects();
    t.move_to(pt(10.0, 10.0));
    assert_eq!(t.effects(), [Effect::Cursor(CursorIcon::NotAllowed)]);
}

#[test]
fn a_widget_that_declines_lets_the_event_bubble_to_the_clickable_parent() {
    struct Decline;
    impl<M: 'static> Widget<M> for Decline {
        type State = ();
        fn create_state(&self) {}
    }
    struct DeclineEl<M: 'static> {
        base: Base<M>,
        widget: Decline,
    }
    builder!(DeclineEl<M> => Decline);

    let mut t = ui(300.0, 100.0);
    t.view(
        column().child(
            column()
                .height(50.0)
                .on_click(Msg::Click("parent"))
                .child(DeclineEl { base: Base::new(), widget: Decline }.key("child").height(20.0)),
        ),
    );
    t.click_key("child");
    assert_eq!(t.messages(), [Msg::Click("parent")]);
}

// ---- a widget whose size changes by itself ----------------------------------------------------

struct Expander;

#[derive(Default)]
struct Extra(f32);

impl Widget<Msg> for Expander {
    type State = Extra;

    fn create_state(&self) -> Extra {
        Extra::default()
    }

    fn update(&self, _: &Expander, _: &mut Extra) -> Dirty {
        Dirty::None
    }

    fn measure(&self, state: &mut Extra, _: &Env, _: Pair<Option<f32>>, _: Pair<Avail>) -> Size {
        size(50.0, 20.0 + state.0)
    }

    fn event(&self, state: &mut Extra, cx: &mut EventCx<Msg>, event: &WidgetEvent) -> bool {
        if let WidgetEvent::Input(Event::PointerDown { .. }) = event {
            state.0 += 10.0;
            cx.request_layout();
            return true;
        }
        false
    }
}

struct ExpanderEl<M: 'static> {
    base: Base<M>,
    widget: Expander,
}
builder!(ExpanderEl<M> => Expander);

#[test]
fn a_widget_can_ask_for_a_new_layout_when_it_changes_size() {
    let mut t = ui(200.0, 200.0);
    t.view(column().align_items(Align::Start).children((
        ExpanderEl { base: Base::new(), widget: Expander }.key("grows"),
        div().key("below").height(10.0),
    )));
    assert_eq!((t.rect("grows").h, t.rect("below").y), (20.0, 20.0));
    t.click_key("grows");
    assert_eq!((t.rect("grows").h, t.rect("below").y), (30.0, 30.0));
    t.click_key("grows");
    assert_eq!((t.rect("grows").h, t.rect("below").y), (40.0, 40.0));
}

// ---- a widget that asks for a part of itself to be brought into view --------------------------

struct Beacon;

impl Widget<Msg> for Beacon {
    type State = ();

    fn create_state(&self) {}

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn event(&self, _: &mut (), cx: &mut EventCx<Msg>, event: &WidgetEvent) -> bool {
        if let WidgetEvent::FocusChanged { focused: true, .. } = event {
            // What matters is far down this tall element, like a caret in a long text.
            cx.scroll_into_view(rect(cx.rect.x, cx.rect.y + 350.0, 10.0, 10.0));
        }
        false
    }
}

struct BeaconEl<M: 'static> {
    base: Base<M>,
    widget: Beacon,
}
builder!(BeaconEl<M> => Beacon);

#[test]
fn a_widget_can_scroll_a_part_of_itself_into_view() {
    let mut t = ui(200.0, 100.0);
    t.view(
        column()
            .key("scroller")
            .overflow(Overflow::Scroll)
            .child(BeaconEl { base: Base::new(), widget: Beacon }.key("tall").height(400.0).shrink(0.0)),
    );
    t.tab().settle();
    // The mark spans 350 to 360. It ends up 8 px above the bottom edge of the 100 px view.
    assert_eq!(t.inspect("scroller").scroll.y, 360.0 + 8.0 - 100.0);
}

// ---- a widget that hears about everything ------------------------------------------------------

struct Spy {
    log: Log,
}

impl<M: 'static> Widget<M> for Spy {
    type State = ();

    fn create_state(&self) {}

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn event(&self, _: &mut (), cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let line = match event {
            WidgetEvent::HoverChanged(on) => format!("hover {on} (cx {})", cx.hovered),
            WidgetEvent::FocusChanged { focused, visible, .. } => format!("focus {focused} visible {visible} (cx {})", cx.focused),
            WidgetEvent::WindowFocus(on) => format!("window {on}"),
            WidgetEvent::Action(action) => format!("action {action:?}"),
            _ => return false,
        };
        self.log.borrow_mut().push(line);
        false
    }
}

struct SpyEl<M: 'static> {
    base: Base<M>,
    widget: Spy,
}
builder!(SpyEl<M> => Spy);

fn spy(log: &Log) -> SpyEl<Msg> {
    SpyEl { base: Base::new(), widget: Spy { log: log.clone() } }
}

#[test]
fn widgets_hear_about_hover_focus_and_the_window() {
    let log = log();
    let mut t = ui(200.0, 100.0);
    t.view(column().child(spy(&log).key("spy").height(30.0)));
    t.move_to(pt(10.0, 10.0));
    t.leave();
    t.tab();
    t.window_focus(false);
    t.key(NamedKey::Enter);
    assert_eq!(
        take(&log),
        [
            "hover true (cx true)",
            "hover false (cx false)",
            "focus true visible true (cx true)",
            "window false",
            "action Click",
        ]
    );
    t.click(pt(10.0, 90.0));
    assert_eq!(take(&log), ["focus false visible false (cx false)"]);
}

#[test]
fn only_the_focused_widget_hears_about_the_window() {
    let log = log();
    let mut t = ui(200.0, 100.0);
    t.view(column().child(spy(&log).key("spy").height(30.0)));
    t.window_focus(false);
    t.window_focus(true);
    assert!(take(&log).is_empty());
}

// ---- painting around the children --------------------------------------------------------------

struct Frame;

impl<M: 'static> Widget<M> for Frame {
    type State = ();

    fn create_state(&self) {}

    fn paint(&self, _: &(), cx: &mut PaintCx) {
        let c = cx.content;
        cx.fill_rect(c, Tone::Wash);
    }

    fn paint_over(&self, _: &(), cx: &mut PaintCx) {
        let c = cx.content;
        cx.fill_rect(rect(c.x, c.y, 3.0, 3.0), Tone::Danger);
    }
}

struct FrameEl<M: 'static> {
    base: Base<M>,
    widget: Frame,
    children: Vec<Element<M>>,
}
builder!(FrameEl<M> => Frame, children);

#[test]
fn a_widget_paints_under_its_children_and_over_them() {
    let mut t = ui(100.0, 100.0);
    t.view(FrameEl::<Msg> { base: Base::new(), widget: Frame, children: Vec::new() }.child(div().size(50.0, 50.0).bg(Tone::Accent)));
    let palette = t.ui.theme().palette.clone();
    let scene = t.scene();
    let fills: Vec<_> = quads(&scene).iter().map(|q| q.fill).collect();
    assert_eq!(fills, [palette.wash, palette.accent, palette.danger]);
}
