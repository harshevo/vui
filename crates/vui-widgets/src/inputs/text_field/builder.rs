//! Building a field: `text_field`, `text_area`, `search_field` and `password_field`, and the
//! methods that configure them.

use super::*;

/// A text field, single line or several.
pub struct TextField<M: 'static> {
    pub(crate) base: Base<M>,
    pub(crate) widget: TextFieldWidget<M>,
}

impl<M: 'static> TextField<M> {
    fn new(value: impl Into<String>, spec: Spec) -> TextField<M> {
        let field = TextField {
            base: Base::new(),
            widget: TextFieldWidget { value: value.into(), placeholder: String::new(), on_change: None, on_submit: None, on_paste_image: None, spec },
        };
        // The field shows its own focus, so the runtime's ring stays off.
        field.bg(Tone::Field).rounded(Radius::Control).focus_ring(false)
    }

    /// Hint shown while the field is empty. It also names the field for assistive technology unless
    /// the element has a name of its own.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.widget.placeholder = text.into();
        self
    }

    /// Sent with the whole new text after every edit. Without it the field edits freely and takes the
    /// value only at the start.
    pub fn on_change(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }

    /// Sent with the text when Return is pressed in a single line, or Command-Return in several.
    pub fn on_submit(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.widget.on_submit = Some(Rc::new(f));
        self
    }

    /// Sent with a picture pasted while the clipboard holds no text, as PNG bytes. Without it, a
    /// picture on the clipboard pastes nothing.
    pub fn on_paste_image(mut self, f: impl Fn(Vec<u8>) -> M + 'static) -> Self {
        self.widget.on_paste_image = Some(Rc::new(f));
        self
    }

    /// Shows bullets for the text and keeps it off the clipboard. One line only.
    pub fn password(mut self) -> Self {
        self.widget.spec.password = true;
        self.widget.spec.multiline = false;
        self
    }

    /// The text can be selected and copied but not changed.
    pub fn read_only(mut self, on: bool) -> Self {
        self.widget.spec.read_only = on;
        self
    }

    pub fn max_chars(mut self, max: usize) -> Self {
        self.widget.spec.max_chars = Some(max);
        self
    }

    /// An icon before the text.
    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.widget.spec.leading = Some(icon);
        self
    }

    /// A button after the text that empties the field, shown while there is text.
    pub fn clearable(mut self) -> Self {
        self.widget.spec.clearable = true;
        self
    }

    /// How many lines tall a multi-line field is at least.
    pub fn rows(mut self, rows: usize) -> Self {
        self.widget.spec.rows = rows.max(1);
        self
    }

    /// Lets a multi-line field grow with its text up to this many lines, and scroll beyond.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.widget.spec.max_rows = Some(rows.max(1));
        self
    }

    pub fn mono(mut self) -> Self {
        self.widget.spec.mono = true;
        self
    }

    /// The taller field for primary inputs.
    pub fn large(mut self) -> Self {
        self.widget.spec.large = true;
        self
    }

    /// Selects everything when focus arrives by anything but a click.
    pub fn select_on_focus(mut self) -> Self {
        self.widget.spec.select_on_focus = true;
        self
    }
}

pub(crate) const FIELD: Spec = Spec {
    multiline: false,
    rows: 1,
    max_rows: None,
    password: false,
    read_only: false,
    search: false,
    clearable: false,
    mono: false,
    large: false,
    select_on_focus: false,
    max_chars: None,
    leading: None,
};

/// A single-line field showing `value`.
pub fn text_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, FIELD)
}

/// A field for several lines. Three tall until `rows` or `max_rows` say otherwise.
pub fn text_area<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { multiline: true, rows: 3, ..FIELD })
}

/// A field for finding things: a magnifier, a button that clears it, and the search role.
pub fn search_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { search: true, clearable: true, leading: Some(set::SEARCH), ..FIELD }).placeholder("Search")
}

pub fn password_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { password: true, ..FIELD })
}
