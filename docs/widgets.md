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
| `number_input(value, min, max, step, Msg::Set)` | a field with − and + |

### Text input

| Widget | Notes |
| --- | --- |
| `text_field(&value)` | `.placeholder`, `.on_change`, `.on_submit`, `.leading_icon`, `.clearable()`, `.max_chars`, `.read_only`, `.select_on_focus()` |
| `text_area(&value)` | several lines: `.rows(4)`, `.max_rows(12)`; `.mono()` for code |
| `search_field(&value)` | a field with a search glyph and a clear button |
| `password_field(&value)` | hidden characters; copying is refused |

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
