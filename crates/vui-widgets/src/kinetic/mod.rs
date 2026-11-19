//! Kinetic controls: small controls that pack several actions into one place and move like
//! objects, with springs instead of fixed-length tweens.
//!
//! | Widget | What it replaces |
//! | --- | --- |
//! | [`segments`] | a row of buttons or radio buttons: one pill glides to the chosen one |
//! | [`slot`] | a dropdown of a few modes: one word that rolls to the next on wheel, drag or click |
//! | [`scrub`] | a number field: drag sideways to change it, click to type it |
//! | [`hold_button`] | an "Are you sure?" dialog: hold to confirm, let go early to cancel |
//! | [`split_button`] | two buttons for one action and its variants |
//! | [`radial_menu`] | a context menu: choices in a ring around the pointer, all the same distance away |
//! | [`tone_chip`] | a status label that is also its own control |
//! | [`shake`] | an error dialog for a bad entry: the field shakes its head |
//!
//! Use them for a handful of choices (two to eight) or for one number. For a long list, a menu
//! with search (the command palette) is still the right control.

mod hold;
mod radial;
mod scrub;
mod segments;
mod slot;
mod split;

use std::rc::Rc;

use vui_core::{TextLayout, TextRole, TextStyle, TextWrap};
use vui_runtime::{Env, Pose};

pub use hold::{Hold, HoldWidget, hold_button};
pub use radial::{RadialWidget, radial_menu};
pub use scrub::{Scrub, ScrubWidget, scrub};
pub use segments::{Segments, SegmentsWidget, segments};
pub use slot::{Slot, SlotWidget, slot};
pub use split::{split_button, tone_chip};

/// A shake of the head, for an entry that was refused: give it to `.animate(..)` on the element and
/// key the element by the attempt, so each refusal plays it afresh.
pub fn shake() -> vui_core::Keyframes<Pose> {
    use vui_core::Easing::EaseOut;
    let at = |x: f32| Pose::REST.translate(x, 0.0);
    vui_core::Keyframes::new(0.42, Pose::REST)
        .to(0.15, at(-6.0), EaseOut)
        .to(0.35, at(5.0), EaseOut)
        .to(0.55, at(-3.0), EaseOut)
        .to(0.75, at(2.0), EaseOut)
        .to(1.0, Pose::REST, EaseOut)
}

/// How long a widget shakes its head when refused, and how far sideways it is `left` seconds before
/// the shake ends: a quick decaying wobble, still with reduced motion asked for.
pub(crate) const SHAKE: f32 = 0.4;

pub(crate) fn shake_offset(left: f32, still: bool) -> f32 {
    if left <= 0.0 || still {
        return 0.0;
    }
    let t = SHAKE - left;
    (t * 48.0).sin() * 4.0 * (left / SHAKE)
}

/// One line of text in the theme's `role`, at `weight`, never wrapped.
pub(crate) fn line(env: &Env, text: &str, role: TextRole, weight: u16, mono: bool) -> Rc<TextLayout> {
    let mut style = TextStyle::from_token(env.theme.types.get(role)).weight(weight);
    if mono {
        style = style.mono();
    }
    style.wrap = TextWrap::None;
    env.fonts().layout_cached(text, &style, None)
}
