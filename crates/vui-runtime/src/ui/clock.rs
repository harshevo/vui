//! Time: frame ticks, the app's timers, and the messages background tasks send back.

use super::*;

impl<M: 'static> Ui<M> {
    /// Seconds until a widget or an app timer wants a `tick` even though nothing is animating (a
    /// blinking caret, `Cx::after`).
    pub fn next_wake(&self) -> Option<f32> {
        let timer = self.timers.iter().map(|(due, _)| *due).reduce(f32::min);
        let at = match (self.core.wake_at, timer) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        at.map(|t| (t - self.core.time).max(0.0))
    }

    /// Has background tasks and `AppSender`s call `waker` once they have sent something, so the
    /// backend wakes its event loop and calls [`Ui::poll`]. Without one, messages wait for the next
    /// `tick`.
    pub fn set_waker(&mut self, waker: Waker) {
        self.tasks.waker = Some(waker);
    }

    /// Hands the app what background tasks and `AppSender`s have sent so far.
    pub fn poll(&mut self) {
        let mut any = false;
        while let Ok(msg) = self.inbox.try_recv() {
            let pending = &self.tasks.pending;
            pending.set(pending.get().saturating_sub(1));
            self.core.out.messages.push(msg);
            any = true;
        }
        if any {
            self.sync();
            self.pump();
            self.sync();
        }
    }

    /// Background tasks (`Cx::spawn`) started and not yet answered.
    pub fn pending_tasks(&self) -> usize {
        self.tasks.pending.get()
    }

    /// Blocks until a background task or `AppSender` sends something, for at most `limit`, and
    /// hands it to the app. False if nothing came. For tests and tools.
    pub fn wait(&mut self, limit: std::time::Duration) -> bool {
        match self.inbox.recv_timeout(limit) {
            Ok(msg) => {
                let pending = &self.tasks.pending;
                pending.set(pending.get().saturating_sub(1));
                self.send(msg);
                self.poll();
                true
            }
            Err(_) => false,
        }
    }

    /// Hands the app the timers that are due.
    pub(crate) fn fire_timers(&mut self) {
        let now = self.core.time + TIMER_SLACK;
        if !self.timers.iter().any(|(due, _)| *due <= now) {
            return;
        }
        let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.timers).into_iter().partition(|(at, _)| *at <= now);
        self.timers = waiting;
        self.core.out.messages.extend(due.into_iter().map(|(_, msg)| msg));
        self.pump();
    }

    /// Seconds of UI time that have passed, as summed from `tick`.
    pub fn time(&self) -> f32 {
        self.core.time
    }

    /// Advances animations by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.devtools.step(dt);
        let core = &mut self.core;
        core.time += dt;
        let mut ts = TickState {
            env: &core.env,
            time: core.time,
            window_focused: core.window_focused,
            focus: core.focus,
            wake: None,
            moving: false,
            layout: false,
            arrange: false,
            paint: false,
        };
        if let Some(root) = self.root.as_mut() {
            root.tick(dt, &mut ts);
        }
        let (moving, wake, layout, arrange, painted) = (ts.moving, ts.wake, ts.layout, ts.arrange, ts.paint);
        core.animating = moving;
        let now = core.time;
        let mut at = wake.map(|s| now + s);
        if let Some(old) = core.wake_at
            && old > now
        {
            at = Some(at.map_or(old, |a| a.min(old)));
        }
        core.wake_at = at;
        core.layout_dirty |= layout;
        core.arrange_dirty |= arrange;
        core.paint_dirty |= painted;
        if core.tip.tick(core.time) {
            core.paint_dirty = true;
        }
        self.sync();
        self.fire_timers();
        self.poll();
    }
}
