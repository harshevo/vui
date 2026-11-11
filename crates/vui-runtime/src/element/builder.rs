//! The builder vocabulary every element shares: decoration, input, identity, accessibility,
//! responsive rules and transitions.

use super::*;

/// The chainable vocabulary every element builder shares, on top of [`Styled`]: decoration, input
/// handlers, identity, accessibility and transitions. Implement `base_mut`, or use [`builder!`](crate::builder).
pub trait Builder<M: 'static>: Styled {
    fn base_mut(&mut self) -> &mut Base<M>;

    /// Names the element so it keeps its state when its siblings reorder, and so the app can find
    /// its rectangle with `Ui::rect_of`.
    fn key(mut self, key: impl Into<Key>) -> Self {
        self.base_mut().key = Some(key.into());
        self
    }

    /// The fill behind the element.
    fn bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.fill = fill.into();
        self
    }

    /// Faded over the fill while the pointer is over the element.
    fn hover_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.hover_fill = fill.into();
        self
    }

    /// Faded over the fill while the element is pressed.
    fn press_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.press_fill = fill.into();
        self
    }

    /// Faded over the fill while files are dragged over the element.
    fn drop_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.drop_fill = fill.into();
        self
    }

    /// A line `width` thick drawn inside the element's edge. Layout ignores it: pad to make room.
    fn border(mut self, width: f32, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.border = Some((width, fill.into()));
        self
    }

    /// A soft shadow under the element, following its rounded corners: `.shadow(Shadow::RAISED)`.
    /// It is stronger in dark mode, where a faint one would not show.
    fn shadow(mut self, shadow: vui_core::Shadow) -> Self {
        self.base_mut().decor.shadow = Some(shadow);
        self
    }

    fn rounded(mut self, radius: impl Into<Radius>) -> Self {
        self.base_mut().decor.radius = radius.into();
        self
    }

    /// Fades the element and everything inside it.
    fn opacity(mut self, opacity: f32) -> Self {
        self.base_mut().decor.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Turns the keyboard focus ring off, for widgets that show focus themselves.
    fn focus_ring(mut self, on: bool) -> Self {
        self.base_mut().decor.focus_ring = on;
        self
    }

    /// Animates changes to the element's size, insets, fill and opacity with `spring`. Insets make an
    /// absolute element slide: a selection marker whose `top` follows the selected row.
    fn transition(mut self, spring: Spring) -> Self {
        self.base_mut().transition = Some(spring);
        self
    }

    /// Animates changes with the theme's usual spring.
    fn animated(self) -> Self {
        self.transition(Spring::SNAPPY)
    }

    /// Fades the element in and lifts it into place when it first shows up, with the `transition`
    /// spring or a smooth one. Key it by what it shows, a page or a row, so that new content appears
    /// afresh instead of updating the old element in place.
    fn appear(mut self) -> Self {
        self.base_mut().appear = true;
        self
    }

    /// Sent when the element is clicked: pressed and released while over it.
    fn on_click(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_click_at(move |_| message.clone())
    }

    fn on_click_with(self, f: impl Fn() -> M + 'static) -> Self {
        self.on_click_at(move |_| f())
    }

    fn on_click_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.click = Some(Box::new(f));
        self
    }

    /// Sent the moment the primary button goes down on the element.
    fn on_press(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_press_at(move |_| message.clone())
    }

    fn on_press_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.press = Some(Box::new(f));
        self
    }

    /// Sent after the second click of a double click. The first click still sends `on_click`.
    fn on_double_click(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_double_click_at(move |_| message.clone())
    }

    fn on_double_click_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.double_click = Some(Box::new(f));
        self
    }

    /// Sent on a right click or control-click: where a context menu opens.
    fn on_secondary(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.secondary = Some(Box::new(f));
        self
    }

    /// Sent when the pointer enters (`true`) or leaves (`false`) the element or anything inside it.
    fn on_hover(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.base_mut().input.hover = Some(Box::new(f));
        self
    }

    /// Sees key presses that reach the element, from itself or anything focused inside it. Return
    /// `None` to let the key carry on to the next handler.
    fn on_key(mut self, f: impl Fn(&KeyEvent) -> Option<M> + 'static) -> Self {
        self.base_mut().input.key = Some(Box::new(f));
        self
    }

    /// Accepts files dropped on the element.
    fn on_drop(mut self, f: impl Fn(Vec<PathBuf>) -> M + 'static) -> Self {
        self.base_mut().input.drop = Some(Box::new(f));
        self
    }

    /// Sends `message` when a press lands outside the element, or Escape is pressed and nothing
    /// focused answers it: how menus, popovers and dialogs close. The press that dismisses goes no
    /// further. With several open, Escape closes the topmost first.
    fn on_dismiss(mut self, message: M) -> Self
    where
        M: Clone,
    {
        self.base_mut().input.dismiss = Some(Box::new(move || message.clone()));
        self
    }

    /// A short line shown under the element after the pointer rests on it for a moment, and read out
    /// as its description: what an icon button does, the full text of something cut short.
    fn tooltip(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        let base = self.base_mut();
        if base.a11y.description.is_none() {
            base.a11y.description = Some(text.clone());
        }
        base.input.tooltip = Some(text);
        self
    }

    /// Takes keyboard focus when the element first appears: the field of a dialog, a menu's first item.
    fn autofocus(mut self) -> Self {
        self.base_mut().input.autofocus = true;
        self
    }

    /// Keeps Tab going round the focusable elements inside this one while it shows, and moves focus
    /// into it when it appears: a dialog.
    fn trap_focus(mut self) -> Self {
        self.base_mut().input.trap_focus = true;
        self
    }

    /// Makes the element focusable by Tab and by click, or not focusable at all.
    fn focusable(self, on: bool) -> Self {
        self.focus_mode(if on { Focus::Click } else { Focus::Never })
    }

    fn focus_mode(mut self, focus: Focus) -> Self {
        self.base_mut().input.focus = Some(focus);
        self
    }

    /// Makes the element a focus group: Tab visits it once, and the arrow keys move between the
    /// focusable elements inside it.
    fn focus_group(mut self, group: FocusGroup) -> Self {
        self.base_mut().input.group = Some(group);
        self
    }

    /// Inside a focus group, the item Tab lands on when focus comes into the group: the selected radio
    /// button, the current page. Without one, Tab returns to the item that had focus last.
    fn tab_default(mut self, on: bool) -> Self {
        self.base_mut().input.tab_default = on;
        self
    }

    /// The pointer shape over the element.
    fn cursor(mut self, cursor: CursorIcon) -> Self {
        self.base_mut().input.cursor = Some(cursor);
        self
    }

    /// Dims the element and stops it, and everything inside it, from taking part in input.
    fn disabled(mut self, disabled: bool) -> Self {
        self.base_mut().input.disabled = disabled;
        self
    }

    /// `false` makes the element see through to what is behind it; its children still get input.
    fn pointer_events(mut self, on: bool) -> Self {
        self.base_mut().input.pointer_events = on;
        self
    }

    /// Dragging the element moves the window, as a title bar does, unless the press lands on something
    /// inside it that takes clicks or focus. For the empty canvas around a window's content.
    fn window_drag(mut self, on: bool) -> Self {
        self.base_mut().input.window_drag = on;
        self
    }

    /// Restyles the element while the window's width is in `when`: a [`Breakpoint`](crate::Breakpoint)
    /// or a [`Width`] range. `f` is given the style so far, and only the properties it changes are
    /// kept, so `.when(Breakpoint::Wide, |s| s.margin_x(pct(15.0)))` changes the side margins on a
    /// wide window and nothing else. The UI follows the window as it is resized.
    fn when(mut self, when: impl Into<Width>, f: impl FnOnce(Restyle) -> Restyle) -> Self {
        let base = self.base_mut();
        crate::responsive::record(&mut base.rules, &base.style, when.into(), f);
        self
    }

    /// What the element is, to assistive technology.
    fn role(mut self, role: Role) -> Self {
        self.base_mut().a11y.role = Some(role);
        self
    }

    /// The name assistive technology reads out, when the visible text is not enough.
    fn name(mut self, name: impl Into<String>) -> Self {
        self.base_mut().a11y.label = Some(name.into());
        self
    }

    /// A name for an element that has none yet: `name` called later or earlier still wins. Widgets
    /// that sit beside a visible label use it to name the control after that label.
    fn default_name(mut self, name: impl Into<String>) -> Self {
        self.base_mut().a11y.label.get_or_insert_with(|| name.into());
        self
    }

    fn describe(mut self, description: impl Into<String>) -> Self {
        self.base_mut().a11y.description = Some(description.into());
        self
    }

    /// Leaves the element and what is inside it out of the accessibility tree.
    fn a11y_hidden(mut self, hidden: bool) -> Self {
        self.base_mut().a11y.hidden = hidden;
        self
    }
}

impl<M: 'static> Styled for Element<M> {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.base.style
    }
}

impl<M: 'static> Builder<M> for Element<M> {
    fn base_mut(&mut self) -> &mut Base<M> {
        &mut self.base
    }
}

impl<M: 'static> Parent<M> for Element<M> {
    fn child_list(&mut self) -> &mut Vec<Element<M>> {
        &mut self.children
    }
}
