# Widgets

Every standard widget is a function that returns a builder. The builder has the widget's own
options (`button("Save").primary()`) plus everything any element can do:

- size and spacing;
- decoration;
- `.key(…)`, `.tooltip(…)` and `.disabled(…)`;
- accessibility names;
- responsive rules.

The gallery (`cargo run -p gallery --release -- --window`) shows them all, in light and dark.

## The pattern: your state, their look

Widgets are **controlled**. The app owns the value and the widget shows it. When the person changes
it, the widget sends a message and your `update` stores the new value:

```rust
use vui::prelude::*;

#[derive(Clone)]
enum Msg { Notify(bool), Name(String) }

struct Prefs { notify: bool, name: String }

fn prefs(p: &Prefs) -> Div<Msg> {
    column().gap(12.0).children((
        toggle(p.notify).name("Notifications").on_change(Msg::Notify),
        text_field(&p.name).placeholder("Your name").on_change(Msg::Name),
    ))
}
```

The same holds for open and closed overlays: `select`, `dropdown`, `popover` and `dialog` take an
`open` flag from your state and a message to send when they should close. Nothing changes behind
your back, and every state can be reproduced in a test.

## Catalogue

### Buttons and actions

| Widget | Notes |
| --- | --- |
| `button("Save")` | `.primary()`, `.secondary()`, `.ghost()`, `.danger()`; `.small()`, `.large()`; `.icon(…)`, `.trailing(…)`; `.loading(true)` |
| `icon_button(icon, "Name")` | a square button with an icon. The name is what screen readers read and what the tooltip shows |
| `segmented(options, &selected, Msg::Pick)` | a segmented control for 2–5 choices |
| `kbd("⌘K")` | a keyboard shortcut chip |

### Controls

| Widget | Notes |
| --- | --- |
| `toggle(on)` | a switch; `.on_change(Msg::Set)` |
| `checkbox(state)` | on, off, or mixed (`CheckState::Mixed`); `.label("…")` |
| `radio(selected)`, `radio_group(options, &selected, Msg::Pick)` | single choice, with arrow keys inside the group |
| `slider(value, min, max)` | `.step(…)`, `.on_change(…)`; arrow keys and Page Up/Down |
| `range_slider(low, high, min, max)` | two thumbs that never cross; `.gap(…)`, `.on_change(\|lo, hi\| …)`; Tab moves from the low thumb to the high one |
| both sliders | `.vertical()`, `.marks()` at every step or `.marks_at(&[…])`, `.show_value()` with `.format(\|v\| …)` |
| `number_input(value, min, max, step, Msg::Set)` | a field with − and + |

### Text input

| Widget | Notes |
| --- | --- |
| `text_field(&value)` | `.placeholder`, `.on_change`, `.on_submit`, `.leading_icon`, `.clearable()`, `.max_chars`, `.read_only`, `.select_on_focus()` |
| `text_area(&value)` | several lines: `.rows(4)`, `.max_rows(12)`; `.mono()` for code |
| `search_field(&value)` | a field with a search glyph and a clear button |
| `password_field(&value)` | hidden characters; copying is refused |
| `combobox(&picker, &options, Msg::City)` | a field with suggestions, filtered by fuzzy matching; Up, Down and Return pick |
| `tag_input(&tags, &draft, Msg::Tags)` | chips plus a field; Return or a comma adds, Backspace removes the last |
| `.pass_keys()`, `.bare()` on a field | leave Up, Down and Return to the parent; draw no outline (for fields inside your own widgets) |

### Pickers

| Widget | Notes |
| --- | --- |
| `calendar(month)` | `.selected(day)`, `.range(a, b)`, `.today(Date::today())`, `.min`, `.max`, `.on_pick`, `.on_month` |
| `date_picker(value, &state, Msg::Due)` | a field that opens a calendar; keep a `DatePicker` |
| `time_field(hour, minute, step, Msg::Time)` | hours and minutes, stepped by buttons or arrow keys |
| `color_picker(color, Msg::Color)` | saturation square, hue strip, hex field and swatches (`.swatches(…)`) |

`Date` is a calendar day with the arithmetic these need: `add_days`, `add_months`, `weekday`,
`iso`, `parse_iso`. A combobox and the command palette share `Picker`, the state of a list that is
filtered by typing:

```rust
use vui::prelude::*;

#[derive(Clone)]
enum Msg { City(PickerEvent) }

struct Trip { city: Picker, chosen: Option<usize> }

const CITIES: [&str; 3] = ["Lisbon", "London", "Lyon"];

impl Trip {
    fn update(&mut self, msg: Msg) {
        match msg {
            // `apply` returns the option's index when one is picked.
            Msg::City(e) => self.chosen = self.city.apply(e).or(self.chosen),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let options: Vec<String> = CITIES.map(String::from).to_vec();
        combobox(&self.city, &options, Msg::City).placeholder("City")
    }
}
```

The editor behind them handles the following:

- **Selection.** By word, line and paragraph.
- **Undo and redo.**
- **The clipboard.** Text and images: `.on_paste_image`.
- **Input methods.** Dead keys, Chinese, Japanese and Korean input, dictation and the emoji
  picker.
- **Large text.** It scrolls.

### Display

| Widget | Notes |
| --- | --- |
| `label`, `title`, `small` | text in the theme's body, title and small styles; see [Text](text-icons-images.md) |
| `markdown(text)`, `rich_text(text)` | headings, lists, emphasis, code and links |
| `badge("New")` | `.tint(BadgeTint::Success)`; Neutral, Accent, Danger, Success, Warning, Orange, Blue, Purple |
| `chip("rust", Some(Msg::Remove))` | a removable tag |
| `avatar("Ada Lovelace")`, `avatar_image(id, 32.0)` | initials on a colour drawn from the name, or a photo |
| `icon(icon_set::BELL)` | any of the 150+ icons; `.color(…)` |
| `image_tile(Some(id))`, `img(id)` | pictures; see [Images](text-icons-images.md#images) |
| `separator()`, `separator_vertical()`, `dot(Tone::Success)` | hairlines and status dots |
| `card()`, `panel()` | surfaces |

### Media

| Widget | Notes |
| --- | --- |
| `web_view(url)` | a web page; WKWebView on macOS |
| `video(url)` | `.playing(…)`, `.looping()`, `.muted(…)`; AVPlayer on macOS, 16:9 by default |
| `pdf_view(path)` | `.page(n)`, counted from 0; PDFKit on macOS |

See [Web pages, video and PDFs](#web-pages-video-and-pdfs).

### Lists, tables and trees

| Widget | Notes |
| --- | --- |
| `list()` + `list_row("Title")` | `.subtitle`, `.leading(…)`, `.trailing("3")`, `.selected(…)`; arrow keys move through the list |
| `item("Label")` | a compact row with `.icon`, `.detail`, `.trailing` |
| `data_table(columns, rows)` | `table_column("Size").numeric().width(96.0).sortable(dir, Msg::Sort)`, `.hide_below(Breakpoint::Medium)`; `table_row(cells).selected(…).on_click(…)` |
| `tree(rows)` + `tree_row(text, depth, expanded, on_toggle)` | outlines and file trees |
| `lasso()` | rubber-band selection over a canvas |

### Navigation

| Widget | Notes |
| --- | --- |
| `sidebar()` + `nav_item(icon, "Inbox")` | `.selected(…)`; `nav_section`, `nav_heading` for groups |
| `tabs(options, &selected, Msg::Tab)` + `tab_panel(content)` | arrow keys move between tabs |
| `breadcrumbs(steps)` | a path whose earlier steps are links |
| `pagination(page, pages, Msg::Page)` | page numbers with gaps |
| `accordion(sections)`, `disclosure(title, open, Msg::Toggle, content)` | collapsible sections |
| `stepper(steps, current)` | progress through a task of several steps; `.on_select` to go back |
| `toolbar(tools)` + `toolbar_separator()` | one Tab stop; Left and Right move between tools |
| `carousel(index, slides, Msg::Slide)` | one slide at a time with arrows, dots and the arrow keys |

### Overlays

| Widget | Notes |
| --- | --- |
| `.tooltip("Text")` on any element | shown after a short hover; follows keyboard focus |
| `popover(anchor, open, Msg::Close, content)` | anchored, dismissed by Escape or a click outside |
| `dropdown(trigger, open, Msg::Close, items)` with `menu_item`, `menu_check`, `menu_heading`, `menu_separator` | a menu |
| `select(options, &selected, open, Msg::Open, Msg::Pick)` | a picker; type to jump |
| `context_menu(target, at, Msg::OpenAt, Msg::Close, items)` | right-click menus |
| `dialog(Msg::Close, content)` + `dialog_actions(buttons)` | modal; traps focus and returns it on close |
| `toasts(items)` + `toast("Saved", Some(icon_set::CHECK))` | stacked notifications; hide them with `cx.after` |
| `command_palette(&picker, &commands, Msg::Palette)` | ⌘K: every `command("New file").group("File").shortcut("⌘N")`, filtered as you type |
| `drawer(Edge::Right, Msg::Close, content)`, `sheet(Msg::Close, content)` | panels from an edge; full width on a phone-sized window |
| `hover_card(anchor, open, Msg::Peek, card, Side::Below)` | rich detail while the pointer rests on something |
| `.keep_focus()` on a popover | leaves focus where it was: a card of suggestions under a field |

### Feedback

| Widget | Notes |
| --- | --- |
| `progress(0.4)`, `progress_indeterminate()` | bars |
| `spinner()` | an activity indicator |
| `alert(AlertKind::Warning, "Title", "Detail")`, `alert_with(…, actions)` | inline messages: Info, Success, Warning, Danger |
| `empty_state(icon, "Title", "Detail", actions)` | a placeholder for empty lists |
| `skeleton(width, height)`, `skeleton_text(lines)` | loading placeholders that shimmer |

### Effects

| Widget | Notes |
| --- | --- |
| `.gradient(Gradient::linear(135.0, [Tone::Accent, Tone::Purple]))` | on any element; also `radial`, `conic`, and `.gradient_border(width, …)` |
| `aurora()` | drifting colour behind the contents; `.colors`, `.period`, `.blob`, `.grain`, `.still` |
| `border_beam(on)` | light running round the edge; `.colors`, `.width`, `.period`, `.length`, `.still` |
| `spotlight()` | lit from the pointer; `.color`, `.reach`, `.at(x, y)` |
| `glow(on)`, `shimmer(on)` | a ring that glows; a sheen that sweeps across |

### Forms

| Widget | Notes |
| --- | --- |
| `field("Email", text_field(&email))` | a labelled control; the label names it for screen readers |
| `setting("Title", "Detail", control)` | a settings row: text on the left, the control on the right |
| `section_title("General")` | a group heading |

## A dialog with a select inside

```rust
use vui::prelude::*;

#[derive(Clone, PartialEq)]
enum Size { S, M, L }

#[derive(Clone)]
enum Msg { Open(bool), PickerOpen(bool), Pick(Size), Save }

struct Order { open: bool, picker: bool, size: Size }

fn order_dialog(o: &Order) -> Div<Msg> {
    let picker = select(
        [(Size::S, "Small".to_string()), (Size::M, "Medium".to_string()), (Size::L, "Large".to_string())],
        &o.size,
        o.picker,
        Msg::PickerOpen,
        Msg::Pick,
    );
    let content = column().gap(16.0).children((
        title("New order"),
        field("Size", picker),
        dialog_actions((
            button("Cancel").ghost().on_click(Msg::Open(false)),
            button("Save").primary().on_click(Msg::Save),
        )),
    ));
    column().children((
        button("New order").on_click(Msg::Open(true)),
        if o.open { dialog(Msg::Open(false), content).into_element() } else { div().into_element() },
    ))
}
```

## Web pages, video and PDFs

`web_view`, `video` and `pdf_view` take a place in layout like any element: size them, round them,
let them grow. They fill the width they are given at their natural shape (a page, 16:9, a letter
page) unless you size them. A URL may be a web address or a file path.

```rust
use vui::prelude::*;

#[derive(Clone)]
enum Msg { Toggle, NextPage }

struct Lesson { playing: bool, page: usize }

fn lesson(l: &Lesson) -> Div<Msg> {
    column().gap(12.0).children((
        video("/Users/me/Movies/lesson.mp4").playing(l.playing).muted(true).rounded(10.0),
        row().gap(8.0).children((
            button(if l.playing { "Pause" } else { "Play" }).on_click(Msg::Toggle),
            button("Next page").on_click(Msg::NextPage),
        )),
        pdf_view("/Users/me/Documents/notes.pdf").page(l.page).height(400.0),
        web_view("https://example.com").height(300.0),
    ))
}
```

On macOS each is a native view (WKWebView, an AVPlayerLayer, PDFKit's `PDFView`) laid over the
window where layout puts it, cut to any scroll view it sits in, with its corners rounded. The view
is kept while the element stays in the tree, so a page keeps its scroll position and a video its
place as layout moves them; changing `.playing` or `.page` updates it in place.

The window draws a placeholder under each: a framed icon and the file or host name, which also
names it for screen readers. That placeholder is all that shows in headless renders, golden images
and on Windows and Linux for now, so goldens stay the same whatever the page holds. Each frame's
list is in `Ui::embeds()`, and the backend hears `Effect::Embeds` only when it changes.

