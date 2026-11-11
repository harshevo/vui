# Accessibility

vui builds an accessibility tree from the same widgets that paint. Every node has a role, a name,
a value and state, and the actions it answers. The keyboard reaches everything a pointer can.

> **Status:** the tree, keyboard navigation and focus handling are complete and tested. The
> bridge that hands the tree to VoiceOver on macOS (through AccessKit or NSAccessibility) is
> not built yet; it is first on the [roadmap](roadmap.md). Until then, screen readers can't see
> vui windows, so keep building the tree right and it will be read out as soon as the bridge
> lands.

## What you get for free

- **Roles and state.** Standard widgets have roles (button, switch, checkbox, slider, text
  input, tab, list item, dialog, menu…), their values and states (checked, selected, expanded,
  ranges), and their actions (click, increment, decrement, set value).
- **Names from visible text.** A button is named by its label and a text field by its
  placeholder, unless something better is given. `field("Email", …)` and `setting("Wi-Fi", …)` name
  the control after its label, so the label isn't read out twice.
- **Keyboard focus.**
  - Tab and Shift-Tab move between controls.
  - The arrow keys move inside lists, tabs, radio groups, segmented controls and menus. Each is
    one Tab stop.
  - Escape closes popovers and dialogs.
  - Dialogs trap focus and give it back when they close.
- **Focus rings.** They show when focus arrives by keyboard, not on clicks.

## What you add

```rust
use vui::prelude::*;

#[derive(Clone)]
enum Msg { Close, Play }

let bar: Div<Msg> = row().gap(8.0).children((
    // Icon-only buttons need a name: it is what is read out, and what the tooltip shows.
    icon_button(icon_set::X, "Close").on_click(Msg::Close),
    // A custom clickable element: give it a role and a name, and let Tab reach it.
    div().square(32.0).rounded(Radius::Pill).bg(Tone::Accent)
        .role(Role::Button).name("Play").focusable(true).on_click(Msg::Play),
    // Decoration that would only add noise.
    icon(icon_set::SPARKLES).a11y_hidden(true),
    // A longer explanation, read after the name.
    toggle(true).name("Sync").describe("Keeps your notes the same on every device"),
));
```

| Method | Use |
| --- | --- |
| `.name("…")` | the name read out, replacing any from visible text |
| `.default_name("…")` | a name unless the element has one |
| `.describe("…")` | a longer description |
| `.role(Role::…)` | what a custom element is |
| `.a11y_hidden(true)` | leaves decoration out |
| `.focusable(true)`, `.focus_group(FocusGroup::vertical())`, `.trap_focus()`, `.autofocus()` | keyboard reach |
| `.tooltip("…")` | a hover hint, which also becomes the description |

Custom widgets describe themselves in `Widget::access`; see the rating example in
[Custom widgets](custom-widgets.md).

## Testing it

```rust
use vui::prelude::*;
use vui::testing::TestUi;

let mut t: TestUi<()> = TestUi::new(300.0, 100.0);
t.view(row().child(icon_button(icon_set::TRASH, "Delete")));
let tree = t.ui.access_tree();
assert!(tree.nodes.iter().any(|n| n.role == Role::Button && n.name.as_deref() == Some("Delete")));
```

`t.ui.do_action(id, AccessAction::Click)` performs an action the way a screen reader would.
