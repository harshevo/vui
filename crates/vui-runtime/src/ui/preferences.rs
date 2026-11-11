//! Following the user's accessibility settings: the theme adapts to higher contrast and larger
//! text, and motion is cut short.

use super::*;

impl<M: 'static> Ui<M> {
    /// Follows `prefs`: the theme is adapted (see [`Theme::adapted`]), springs and transitions jump
    /// to their ends, keyframes hold their first pose, and effects hold still. Windows set this from
    /// the system's settings on their own; apps and tests can set it too.
    pub fn set_preferences(&mut self, prefs: vui_core::Preferences) {
        if prefs == self.core.env.prefs {
            return;
        }
        self.core.env.prefs = prefs;
        let designed = (*self.core.env.designed).clone();
        self.set_theme(designed);
        if prefs.reduce_motion {
            // One tick lets everything moving reach its end.
            self.core.animating = true;
        }
    }

    pub fn preferences(&self) -> vui_core::Preferences {
        self.core.env.prefs
    }
}
