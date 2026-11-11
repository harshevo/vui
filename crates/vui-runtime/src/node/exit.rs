//! Leaving: an element marked `disappear` that the view drops stays as a ghost while it fades out
//! and sinks, then goes. Ghosts take no pointer, keyboard or focus, and are left out of the
//! accessibility tree.

use super::*;

impl<M: 'static> Node<M> {
    /// Gone as far as input, focus and assistive technology are concerned: hidden, or leaving.
    pub(crate) fn gone(&self) -> bool {
        self.eff.hidden || self.leaving
    }

    /// Starts fading out.
    pub(crate) fn leave(&mut self) {
        self.leaving = true;
        let spring = self.transition.unwrap_or(Spring::SMOOTH);
        self.anim.opacity.spring = spring;
        self.anim.opacity.set_target(0.0);
        let mut rise = self.anim.rise.take().unwrap_or_else(|| SpringValue::new(0.0, spring));
        rise.spring = spring;
        rise.set_target(RISE);
        self.anim.rise = Some(rise);
    }

    /// Back in the view before it had gone: fades back in.
    pub(crate) fn revive(&mut self) {
        self.leaving = false;
        self.anim.opacity.set_target(self.decor.opacity);
        if let Some(rise) = self.anim.rise.as_mut() {
            rise.set_target(0.0);
        }
    }

    /// Has faded out and can be removed.
    pub(crate) fn left(&self) -> bool {
        self.leaving && self.anim.opacity.is_settled() && self.anim.opacity.value() <= 0.001
    }
}
