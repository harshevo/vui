//! Buttons: pills with a label, an icon or both, in four weights and three sizes.

use vui_core::{Fill, TextRole, Tone};
use vui_icons::Icon;
use vui_runtime::{Base, Builder, Radius, Role, Styled, builder};

use crate::data::item::{Height, ItemWidget, Spec};

/// How much a button stands out. A window has one [`Primary`](ButtonVariant::Primary) at most.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// The accent: the one thing to do here.
    Primary,
    /// A soft grey fill. The default.
    Secondary,
    /// No box until the pointer is over it: toolbars and rows.
    Ghost,
    /// Irreversible actions only.
    Danger,
}

/// A button's height and the size of its text.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ButtonSize {
    /// 24 high, small text.
    Small,
    /// 32 high.
    Medium,
    /// 40 high.
    Large,
}

/// A button. Pressing it, or Space or Return while it has focus, sends the `on_click` message.
pub struct Button<M: 'static> {
    base: Base<M>,
    widget: ItemWidget<M>,
}

builder!(Button<M> => ItemWidget<M>);

impl<M: 'static> Button<M> {
    fn new(label: String) -> Button<M> {
        let mut spec = Spec::new(label, Role::Button);
        spec.weight = 500;
        let button = Button { base: Base::new(), widget: ItemWidget::new(spec) };
        button.rounded(Radius::Pill).variant(ButtonVariant::Secondary).size(ButtonSize::Medium).cursor(vui_core::CursorIcon::Pointer)
    }

    /// How much the button stands out.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        let spec = &mut self.widget.spec;
        let (label, icon) = match variant {
            ButtonVariant::Primary => (Tone::OnAccent, Tone::OnAccent),
            ButtonVariant::Danger => (Tone::OnDanger, Tone::OnDanger),
            ButtonVariant::Secondary | ButtonVariant::Ghost => (Tone::Text, Tone::Text2),
        };
        spec.label_color = label.into();
        spec.icon_color = icon.into();
        spec.detail_color = match variant {
            ButtonVariant::Primary | ButtonVariant::Danger => Fill::Tone(label, 0.7),
            ButtonVariant::Secondary | ButtonVariant::Ghost => Tone::Text3.into(),
        };
        match variant {
            ButtonVariant::Primary => self.bg(Tone::Accent).hover_bg(Fill::Tone(Tone::Text, 0.10)).press_bg(Fill::Tone(Tone::Text, 0.20)).border(0.0, Fill::NONE),
            ButtonVariant::Danger => self.bg(Tone::Danger).hover_bg(Fill::Tone(Tone::Text, 0.10)).press_bg(Fill::Tone(Tone::Text, 0.20)).border(0.0, Fill::NONE),
            ButtonVariant::Secondary => self.bg(Tone::Wash).hover_bg(Tone::Hover).press_bg(Tone::Wash).border(0.0, Fill::NONE),
            ButtonVariant::Ghost => self.bg(Fill::NONE).hover_bg(Tone::Hover).press_bg(Tone::Wash).border(0.0, Fill::NONE),
        }
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        let spec = &mut self.widget.spec;
        let (height, text, pad) = match size {
            ButtonSize::Small => (Height::Small, TextRole::Small, 8.0),
            ButtonSize::Medium => (Height::Control, TextRole::Body, 12.0),
            ButtonSize::Large => (Height::Large, TextRole::Body, 16.0),
        };
        spec.height = height;
        spec.text = text;
        self.padding_x(pad)
    }

    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }

    pub fn large(self) -> Self {
        self.size(ButtonSize::Large)
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.widget.spec.icon = Some(icon);
        self
    }

    /// An icon after the label, such as a chevron on a button that opens something.
    pub fn trailing(mut self, icon: Icon) -> Self {
        self.widget.spec.trailing = Some(icon);
        self
    }

    /// Shows a spinner in place of the icon and ignores presses until it is cleared.
    pub fn loading(mut self, loading: bool) -> Self {
        self.widget.spec.busy = loading;
        self
    }

    /// Makes the button as wide as it is tall, for an icon on its own. Give it a `.name(..)`.
    pub fn square_icon(mut self) -> Self {
        self.widget.spec.square = true;
        self.padding_x(0.0)
    }
}

/// A button with a label. Secondary unless told otherwise: `button("Save").primary().on_click(Msg::Save)`.
pub fn button<M: 'static>(label: impl Into<String>) -> Button<M> {
    Button::new(label.into())
}

/// A button that is only an icon. `name` is what a screen reader says: the icon cannot say it.
pub fn icon_button<M: 'static>(icon: Icon, name: impl Into<String>) -> Button<M> {
    Button::new(String::new()).icon(icon).ghost().square_icon().name(name)
}
