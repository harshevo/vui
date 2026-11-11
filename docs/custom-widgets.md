# Custom widgets

Most screens are compositions: rows, columns and the standard widgets, with helper functions
that return a `Div`. Write a widget of your own when you need to:

- **draw something new**, such as a chart, a canvas, a waveform or a rating;
- **handle input in a way no widget does**, such as drawing, dragging handles or custom keyboard
  control;
- **keep state across frames** that the app shouldn't hold, such as hover positions or animations
  in flight.

Every standard widget is built this way, with nothing private: the same trait, the same paint
calls, the same tests.

## Anatomy

A widget is two types:

- **The widget**, which implements `Widget<M>`. It is built fresh by every `view` and holds what
  the app passed in.
- **The builder**, which carries a `Base<M>` with style, decoration, input handlers and
  accessibility. `builder!` gives it the whole element vocabulary (`.width`, `.key`, `.tooltip`,
  `.when`, …).

The runtime keeps a `State` per node between frames. `update` compares the new widget with the
old one and says how much must be redone (`Dirty::None`, `Paint` or `Layout`).

| Method | Called | Default |
| --- | --- | --- |
| `create_state` | when the node first appears | (required) |
| `update(old, state)` | when `view` produced the widget again | `Dirty::Layout` |
| `measure(state, env, known, avail)` | during layout, for its natural size | zero size |
| `paint(state, cx)` / `paint_over` | to draw; under and over its children | nothing |
| `event(state, cx, event)` | for input aimed at it; return `true` to stop bubbling | `false` |
| `tick(state, cx, dt)` / `animating` | every frame while it animates | not animating |
| `access(state, info)` | to describe it to assistive technology | nothing |
| `focusable()` | whether Tab and clicks focus it | `Focus::Never` |
| `cursor()` | the cursor over it | the element's |

## A star rating

This widget draws five stars, sets the rating on a click, answers ← and → when focused, and
reads as a slider to screen readers:

```rust
use std::rc::Rc;

use vui::core::{Event, Key, NamedKey, Size, rect, size};
use vui::prelude::*;
use vui::runtime::layout::{Avail, Pair};
use vui::runtime::{AccessActionKind, AccessInfo, AccessRange, Base, Dirty, Env, EventCx, PaintCx, Widget, WidgetEvent, builder};
use vui::widgets::paint_icon;

const STAR: f32 = 20.0;
const GAP: f32 = 4.0;

pub struct RatingWidget<M> {
    value: u8,
    on_change: Option<Rc<dyn Fn(u8) -> M>>,
}

impl<M: 'static> RatingWidget<M> {
    fn set(&self, cx: &mut EventCx<M>, value: u8) {
        if let Some(f) = &self.on_change {
            cx.emit(f(value.clamp(1, 5)));
        }
    }
}

impl<M: 'static> Widget<M> for RatingWidget<M> {
    type State = ();

    fn create_state(&self) {}

    // Only the value changes what is drawn; the size never changes.
    fn update(&self, old: &Self, _state: &mut ()) -> Dirty {
        if self.value != old.value { Dirty::Paint } else { Dirty::None }
    }

    fn measure(&self, _state: &mut (), _env: &Env, _known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        size(STAR * 5.0 + GAP * 4.0, STAR)
    }

    fn paint(&self, _state: &(), cx: &mut PaintCx) {
        for i in 0..5u8 {
            let at = rect(cx.content.x + i as f32 * (STAR + GAP), cx.content.y, STAR, STAR);
            let tone = if i < self.value { Tone::Warning } else { Tone::Line };
            paint_icon(cx, icon_set::STAR, at, tone, Weight::Medium);
        }
    }

    fn event(&self, _state: &mut (), cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerDown { pos, .. }) => {
                let x = cx.to_local(*pos).x;
                self.set(cx, (x / (STAR + GAP)) as u8 + 1);
                true
            }
            WidgetEvent::Input(Event::Key(k)) if k.pressed => match k.key {
                Key::Named(NamedKey::ArrowRight) => { self.set(cx, self.value + 1); true }
                Key::Named(NamedKey::ArrowLeft) => { self.set(cx, self.value.saturating_sub(1)); true }
                _ => false,
            },
            _ => false,
        }
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn access(&self, _state: &(), info: &mut AccessInfo) {
        info.role = Some(Role::Slider);
        info.value = Some(format!("{} of 5 stars", self.value));
        info.range = Some(AccessRange { min: 1.0, max: 5.0, step: 1.0, value: self.value as f64 });
        info.actions.extend([AccessActionKind::Increment, AccessActionKind::Decrement]);
    }
}

pub struct Rating<M: 'static> {
    base: Base<M>,
    widget: RatingWidget<M>,
}

builder!(Rating<M> => RatingWidget<M>);

impl<M: 'static> Rating<M> {
    pub fn on_change(mut self, f: impl Fn(u8) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }
}

pub fn rating<M: 'static>(value: u8) -> Rating<M> {
    Rating { base: Base::new(), widget: RatingWidget { value, on_change: None } }.cursor(CursorIcon::Pointer)
}

// It is an element like any other:
#[derive(Clone)]
enum Msg { Rate(u8) }
let view: Div<Msg> = row().gap(12.0).children((label("Your rating"), rating(4).name("Rating").on_change(Msg::Rate)));

// And it is tested like any other: a click on the last star rates five.
let mut t: vui::testing::TestUi<Msg> = vui::testing::TestUi::new(300.0, 100.0);
t.view(row().padding(10.0).child(rating(1).key("stars").on_change(Msg::Rate)));
let stars = t.rect("stars");
t.click(pt(stars.right() - 4.0, stars.y + 10.0));
assert!(matches!(t.messages()[..], [Msg::Rate(5)]));
```

### Things to know

- **Coordinates.** `cx.rect` and `cx.content` are in window coordinates. `cx.to_local(pos)` makes a
  pointer position relative to the content box.
- **Colours.** Paint with tones (`Tone::Accent`), not fixed colours, so the widget follows the
  theme and dark mode. `cx.color(fill)` resolves one when you need the `Color`.
- **Drawing.** `PaintCx` has quads (`fill_rect`, `fill_rounded`, `quad`), text (`text` with a
  layout from `cx.fonts().layout_cached(…)`), images (`image`), icons (`paint_icon`) and clipping
  (`clipped`). For paths, strokes and fills, `vui::viz::canvas::Canvas` gives the vector engine
  (`vector().polyline(…)`, fills, dots) and hands the triangles to the scene.
- **Dragging.** Call `cx.capture_pointer()` on press so every move reaches you until release.
- **Animation.** Return `true` from `animating` while something moves, and advance it in `tick`
  with `dt`. Use `Spring` from `vui::core` for natural motion. vui draws frames only while
  something animates.
- **Asking for work.** `cx.request_paint()`, `cx.request_layout()` and `cx.request_wake(seconds)`
  ask for a repaint, a relayout, or a frame later (a caret blink, a timer).

## Custom GPU passes

For effects that are better as a shader (a live gradient, a blur, a particle field, an audio
visualiser), register a **custom pass** and draw it from a widget:

1. **Implement `vui::render::CustomPass`.** `prepare(device, queue, viewport, scale, commands)`
   runs once a frame with every command for your pass. `draw(render_pass, index)` draws the
   `index`-th one, in paint order with everything else.
2. **Register it.** On macOS that is `Application::pass("my-pass", |device, format| Box::new(MyPass::new(device, format)))`.
3. **Draw it.** In your widget's `paint`, push `cx.custom(CustomCmd { pass: "my-pass", rect, clip, opacity, data })`,
   where `data: Vec<f32>` is whatever your shader needs.

vui's own shadows, glows and shimmers are built exactly this way; see
`crates/vui-render/src/effects.rs` and `crates/vui-core/src/effects.rs`. Gradients, the aurora, the
border beam and the spotlight are a second built-in pass, `vui.shading`: before writing a shader
of your own, see whether `vui::core::shading::{fill, border, aurora, beam, spotlight, grain}`
already make the command you want, since those also draw on the CPU. A command for a pass the
renderer doesn't have is skipped. The CPU renderer draws the built-in effects itself, and skips
custom passes it doesn't know.
