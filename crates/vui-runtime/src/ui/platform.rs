//! The app's requests of the system going out as effects, and the system's answers coming back as
//! the app's messages.

use std::path::PathBuf;

use super::*;
use crate::app::Request;
use crate::platform::{Answer, MenuAction, MenuSpec, Notification, NotificationResponse, specs};

/// What to send the app when a request is answered.
enum Reply<M> {
    Paths(Box<dyn FnOnce(Vec<PathBuf>) -> M>),
    Notification(Box<dyn FnOnce(NotificationResponse) -> M>),
}

pub(crate) struct Platform<M> {
    next: u64,
    /// Dialogs and notifications shown and not yet answered, by id.
    replies: HashMap<u64, Reply<M>>,
    /// Notifications waiting for an answer, oldest first.
    notifications: Vec<(u64, Notification)>,
    menus: Vec<MenuSpec>,
    status: Option<MenuSpec>,
    actions: HashMap<u64, MenuAction<M>>,
    /// The native views last sent.
    embeds: Vec<vui_core::Embed>,
}

impl<M> Default for Platform<M> {
    fn default() -> Platform<M> {
        Platform { next: 0, replies: HashMap::new(), notifications: Vec::new(), menus: Vec::new(), status: None, actions: HashMap::new(), embeds: Vec::new() }
    }
}

impl<M: 'static> Ui<M> {
    pub(crate) fn request(&mut self, requests: Vec<Request<M>>) {
        for request in requests {
            let p = &mut self.platform;
            let effect = match request {
                Request::Open(dialog, reply) => {
                    p.next += 1;
                    p.replies.insert(p.next, Reply::Paths(reply));
                    Effect::Open { id: p.next, dialog }
                }
                Request::Save(dialog, reply) => {
                    p.next += 1;
                    p.replies.insert(p.next, Reply::Paths(Box::new(move |paths: Vec<PathBuf>| reply(paths.into_iter().next()))));
                    Effect::Save { id: p.next, dialog }
                }
                Request::Notify(notification, reply) => {
                    p.next += 1;
                    if let Some(reply) = reply {
                        p.replies.insert(p.next, Reply::Notification(reply));
                        p.notifications.push((p.next, notification.clone()));
                    }
                    Effect::Notify { id: p.next, notification }
                }
                Request::FullScreen(on) => Effect::FullScreen(on),
            };
            self.core.out.effects.push(effect);
        }
    }

    /// Asks the hosted app for its menus and menu-bar item, and sends them out if they changed.
    pub(crate) fn refresh_menus(&mut self, driver: &dyn Driver<M>) {
        let mut next = 1;
        let mut actions = Vec::new();
        let menus: Vec<MenuSpec> =
            driver.menus().into_iter().map(|m| MenuSpec { title: m.title, items: specs(m.items, &mut next, &mut actions) }).collect();
        let status = driver.status_item().map(|s| MenuSpec { title: s.title, items: specs(s.items, &mut next, &mut actions) });
        let p = &mut self.platform;
        p.actions = actions.into_iter().collect();
        if menus != p.menus {
            p.menus = menus.clone();
            self.core.out.effects.push(Effect::Menus(menus));
        }
        if status != p.status {
            p.status = status.clone();
            self.core.out.effects.push(Effect::StatusItem(status));
        }
    }

    /// The system's answer to what `Effect::Open`, `Effect::Save` or `Effect::Notify` asked with
    /// `id`: the paths picked (none if the dialog was cancelled), or a [`NotificationResponse`].
    /// The app gets the message it asked for, once; an answer of the wrong kind is ignored.
    pub fn answer(&mut self, id: u64, answer: impl Into<Answer>) {
        let p = &mut self.platform;
        let msg = match (p.replies.remove(&id), answer.into()) {
            (Some(Reply::Paths(reply)), Answer::Paths(paths)) => reply(paths),
            (Some(Reply::Notification(reply)), Answer::Notification(response)) => {
                p.notifications.retain(|(n, _)| *n != id);
                reply(response)
            }
            (Some(reply), _) => {
                p.replies.insert(id, reply);
                return;
            }
            (None, _) => return,
        };
        self.send(msg);
    }

    /// The notifications sent with `Cx::notify_then` and not yet answered, with the ids to answer
    /// them by, oldest first.
    pub fn notifications(&self) -> &[(u64, Notification)] {
        &self.platform.notifications
    }

    /// The person chose the menu item with `id`, from the app's menus or its menu-bar item.
    pub fn choose_menu(&mut self, id: u64) {
        if let Some(action) = self.platform.actions.get(&id).cloned() {
            self.send(action());
        }
    }

    /// The app's menus as last sent, for a backend putting them back when the window comes to the
    /// front, and for tests.
    pub fn menus(&self) -> &[MenuSpec] {
        &self.platform.menus
    }

    /// Sends the native views a new frame asks for, if they changed.
    pub(crate) fn note_embeds(&mut self, scene: &Scene) {
        if scene.embeds != self.platform.embeds {
            self.platform.embeds = scene.embeds.clone();
            self.core.out.effects.push(Effect::Embeds(scene.embeds.clone()));
        }
    }

    /// The native views (web pages, videos, PDFs) as last painted, for backends and tests.
    pub fn embeds(&self) -> &[vui_core::Embed] {
        &self.platform.embeds
    }

    /// The app's menu-bar item as last sent.
    pub fn status_item(&self) -> Option<&MenuSpec> {
        self.platform.status.as_ref()
    }
}
