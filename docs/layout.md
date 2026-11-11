# Layout

vui lays out with flexbox, written from scratch for vui and tuned for interfaces. If you know CSS
flexbox you already know most of it. This guide covers boxes, sizes, alignment, scrolling, layers
and responsive layouts.

## Boxes

Everything is a box:

- `row()` lays its children out left to right.
- `column()` lays them out top to bottom.
- `div()` is a plain box: a column, with nothing set.
- `spacer()` takes the free room, which pushes whatever follows it to the far end.

```rust
use vui::prelude::*;

let toolbar: Div<()> = row().height(44.0).padding_x(12.0).gap(8.0).align_items(Align::Center).children((
    icon_button(icon_set::SIDEBAR, "Toggle sidebar"),
    label("Inbox").weight(600),
    spacer(),
    button("New").primary().icon(icon_set::PLUS),
));
```

Children are given as a tuple (any mix of element types), a `Vec`, an array or an iterator
collected into a `Vec`. `.child(x)` adds one at a time.

## Sizes

Lengths are points (logical pixels) by default, a percentage of the parent with `pct(…)`, or
`AUTO`.

| Method | Effect |
| --- | --- |
| `.width(320.0)`, `.height(pct(50.0))`, `.size(w, h)`, `.square(24.0)` | fixed size |
| `.min_width`, `.max_width`, `.min_height`, `.max_height` | bounds: a card that grows to 560 and no more |
| `.fill_width()`, `.fill_height()`, `.fill()` | 100% of the parent |
| `.grow(1.0)` | takes a share of the free room on the main axis |
| `.shrink(0.0)` | refuses to shrink below its natural size |
| `.basis(220.0)` | the size it starts from before growing or shrinking |
| `.flex(1.0)` | grow and shrink equally from a zero basis: equal columns |
| `.aspect_ratio(16.0 / 9.0)` | keeps a shape as the width changes |

A text label that should shrink and wrap (or truncate with `.max_lines(1)`) inside a row needs
`.min_width(0.0)`. As in CSS, a flex item won't shrink below its content otherwise.

## Spacing and alignment

- **Spacing.** `.padding(16.0)`, `.padding_x`, `.padding_y` and each side on its own; the same
  for `.margin…`. `.gap(8.0)` spaces children evenly. Use the 4/8-point scale: 4, 8, 12, 16, 20,
  24, 32.
- **Cross-axis alignment.** `.align_items(Align::Center)` places children on the cross axis: `Start`,
  `Center`, `End`, `Stretch` (the default) or `Baseline` (rows of text).
- **Main-axis alignment.** `.justify(Justify::SpaceBetween)` distributes them on the main axis.
- **Centring.** `.center()` centres both ways.
- **Wrapping.** `.wrap()` flows children onto new lines when they don't fit, like a tag list or
  a row of cards.
- **Auto margins.** `.margin_left_auto()` pushes an element to the far end, as in CSS.

## Scrolling and clipping

`.overflow(Overflow::Scroll)` makes a box scroll along its direction, and `Overflow::Hidden` clips
without scrolling. A scrolling column needs a bounded height: give it `.grow(1.0).min_height(0.0)`
inside a column that fills the window.

```rust
use vui::prelude::*;

fn page(body: Div<()>) -> Div<()> {
    column().fill().children((
        row().height(52.0).padding_x(16.0).align_items(Align::Center).child(title("Notes")),
        column().key("scroller").grow(1.0).min_height(0.0).overflow(Overflow::Scroll).padding(16.0).child(body),
    ))
}
```

### Long lists

`virtual_list(count, row_height, |i| row)` is a scrolling column that builds only the rows in view,
plus a few either side. A list of 100,000 rows costs what the twenty on screen do, and rows keep
their state while they stay in view.

```rust
use vui::prelude::*;

fn inbox(subjects: std::rc::Rc<Vec<String>>) -> VirtualList<()> {
    virtual_list(subjects.len(), 32.0, move |i| label(subjects[i].clone()).padding_x(12.0))
        .grow(1.0)
        .min_height(0.0)
}
```

Every row is `row_height` tall. Put the spacing inside the rows: the list's own gap isn't used.

- **Following the end.** `.follow_end()` keeps a scroller pinned to the bottom while content
  arrives (a chat log), until the person scrolls up.
- **Keeping things visible.** Scrolling keeps the focused element in view when Tab moves to it.

## Grids

`grid(columns)` places its children into cells, left to right and top to bottom. Each row is as
tall as its tallest cell.

| Columns | Means |
| --- | --- |
| `grid(3)` | three equal columns |
| `grid([Track::Px(240.0), fr(1.0), fr(2.0)])` | a 240-point column, then the rest split one to two |
| `grid([Track::Auto, fr(1.0)])` | a column as wide as its widest cell, then the rest |
| `grid(Columns::fill(200.0))` | as many columns at least 200 wide as fit, and at least one |

```rust
use vui::prelude::*;

let gallery: Div<()> = grid(Columns::fill(180.0)).gap(12.0).children((
    column().span(2).height(120.0).bg(Tone::Wash), // a wide feature tile
    column().height(120.0).bg(Tone::Wash),
    column().height(120.0).bg(Tone::Wash),
));

let form: Div<()> = grid([Track::Auto, fr(1.0)]).gap_x(12.0).gap_y(8.0).align_items(Align::Center).children((
    label("Name"), text_field(""),
    label("Email"), text_field(""),
));
```

- `.span(n)` takes `n` columns. A span that doesn't fit the rest of a row starts the next one.
- `.gap_x` and `.gap_y` set the space between columns and between rows; `.gap` sets both.
- `align_items` and `align_self` place a cell in its row's height. The default is to stretch.
- `.grid_rows(h)` makes every row `h` tall.
- The columns can change at a breakpoint: `grid(1).when(Breakpoint::Medium.up(), |s| s.grid(3))`.

## Layers: absolute, fixed and floating

- `.absolute().top(8.0).right(8.0)` places a child against its parent, out of the flow: a badge
  on an avatar, a close button.
- `.fixed()` places it against the window.
- `.float()` lifts it above everything else: menus, popovers and tooltips use it, so they
  aren't clipped by a scrolling parent.

The overlay widgets (`popover`, `dropdown`, `select`, `dialog`, `toasts`, tooltips) handle this for
you.

## Responsive layouts

Windows come in every size, from a phone-width panel beside other apps to a full display. vui
handles this in the layout itself, so your view doesn't need to know the window size.

`.when(range, |s| …)` restyles an element while the window's width is in `range`, like a CSS media
query:

| Breakpoint | Window width |
| --- | --- |
| `Breakpoint::Compact` | under 600 |
| `Breakpoint::Medium` | 600 to 900 |
| `Breakpoint::Wide` | 900 and up |
| `Width::below(480.0)`, `Width::at_least(1200.0)`, `Width::between(600.0, 1200.0)` | any range you like |

```rust
use vui::prelude::*;

fn shell(sidebar: Div<()>, content: Div<()>) -> Div<()> {
    row().fill().children((
        // The sidebar gives way on a narrow window.
        sidebar.when(Breakpoint::Compact, |s| s.hidden(true)),
        content
            .grow(1.0)
            .min_width(0.0)
            .padding(32.0)
            .when(Breakpoint::Compact, |s| s.padding(12.0))
            .when(Breakpoint::Wide, |s| s.margin_x(pct(10.0))),
    ))
}
```

The closure gets the style vocabulary: sizes, spacing, direction, alignment, wrapping and
`hidden`. A rule changes only what it sets; later rules win where they overlap. vui re-applies the
rules on every resize without calling `view`.

### Container queries

`.when` follows the window. `.when_container` follows the width the element itself is laid out in,
so a component fits wherever it is put: a wide main pane, a narrow sidebar, a dialog.

```rust
use vui::prelude::*;

// Picture beside the text when the card has room, above it when it doesn't.
fn card() -> Div<()> {
    row().gap(12.0).when_container(Width::below(320.0), |s| s.column()).children((
        div().size(80.0, 80.0).bg(Tone::Wash).rounded(8.0),
        column().grow(1.0).children((title("Lisbon"), small("Flights, hotel and a table for two"))),
    ))
}
```

The rules are matched against the last layout. When one starts or stops matching, the tree is laid
out again in the same frame, so nothing flickers.

### Patterns that adapt on their own

You often don't need a breakpoint at all.

- **Cards that wrap.** In a `.wrap()` row, give each card `.grow(1.0).basis(240.0).min_width(240.0)`.
  They share a row when there is room and stack when there isn't. The dashboard example does
  this.
- **A readable column.** `.width(pct(100.0)).max_width(720.0)` inside a parent with
  `.align_items(Align::Center)` fills a narrow window and stops at a comfortable line length on a
  wide one.
- **Rows to columns.** `.when(Breakpoint::Compact, |s| s.column())` turns a side-by-side form into
  a stacked one.
- **Tables.** `table_column("Store").hide_below(Breakpoint::Medium)` drops a column that isn't
  essential.
- **Splitters.** `splitter()` is a drag handle between two panes. `.on_drag(|dx| …)` reports the movement; clamp the pane's width in `update`.

The `examples/dashboard.rs` and `examples/todo.rs` apps are tested at both a wide and a
phone-sized width; see their golden images in `crates/vui/tests/golden`.
