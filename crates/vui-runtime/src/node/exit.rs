//! Leaving: an element marked `disappear` that the view drops stays as a ghost while it fades out
//! and sinks, then goes; one marked `elastic` while it slides out across its side. Ghosts take no
//! pointer, keyboard or focus, and are left out of the accessibility tree.

use super::*;

impl<M: 'static> Node<M> {
    /// Gone as far as input, focus and assistive technology are concerned: hidden, or leaving.
    pub(crate) fn gone(&self) -> bool {
        self.eff.hidden || self.leaving
    }

    /// Starts fading out, or sliding out.
    pub(crate) fn leave(&mut self) {
        self.leaving = true;
        if self.elastic.is_some() {
            let from = self.anim.slide.as_ref().map_or(0.0, SpringValue::value);
            let mut slide = slide(from, 1.0, Spring::ELASTIC_AWAY);
            slide.set_velocity(self.anim.slide.as_ref().map_or(0.0, SpringValue::velocity));
            self.anim.slide = Some(slide);
            if !self.disappear {
                return;
            }
        }
        let spring = self.transition.unwrap_or(Spring::SMOOTH);
        self.anim.opacity.spring = spring;
        self.anim.opacity.set_target(0.0);
        let mut rise = self.anim.rise.take().unwrap_or_else(|| SpringValue::new(0.0, spring));
        rise.spring = spring;
        rise.set_target(RISE);
        self.anim.rise = Some(rise);
    }

    /// Back in the view before it had gone: fades back in, or slides back.
    pub(crate) fn revive(&mut self) {
        self.leaving = false;
        if let Some(slide) = self.anim.slide.as_mut() {
            slide.spring = Spring::ELASTIC;
            slide.set_target(0.0);
        }
        self.anim.opacity.set_target(self.decor.opacity);
        if let Some(rise) = self.anim.rise.as_mut() {
            rise.set_target(0.0);
        }
    }

    /// Has faded out, or slid all the way out, and can be removed.
    pub(crate) fn left(&self) -> bool {
        if !self.leaving {
            return false;
        }
        if self.elastic.is_some() {
            // Gone the first time it is all the way out, before a spring could bring a sliver back.
            if self.anim.slide.as_ref().is_none_or(|s| s.value() >= 0.999) {
                return true;
            }
            if !self.disappear {
                return false;
            }
        }
        self.anim.opacity.is_settled() && self.anim.opacity.value() <= 0.001
    }
}
