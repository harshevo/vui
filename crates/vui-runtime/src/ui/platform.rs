//! The app's requests of the system going out as effects, and the system's answers coming back as
//! the app's messages.

use std::path::PathBuf;

use super::*;
use crate::app::Request;
use crate::platform::{MenuAction, MenuSpec, specs};

type Reply<M> = Box<dyn FnOnce(Vec<PathBuf>) -> M>;

pub(crate) struct Platform<M> {
    next: u64,
    /// Dialogs shown and not yet answered, by id.
    replies: HashMap<u64, Reply<M>>,
    menus: Vec<MenuSpec>,
    status: Option<MenuSpec>,
    actions: HashMap<u64, MenuAction<M>>,
}

impl<M> Default for Platform<M> {
    fn default() -> Platform<M> {
        Platform { next: 0, replies: HashMap::new(), menus: Vec::new(), status: None, actions: HashMap::new() }
    }
}

impl<M: 'static> Ui<M> {
    pub(crate) fn request(&mut self, requests: Vec<Request<M>>) {
        for request in requests {
            let p = &mut self.platform;
            let effect = match request {
                Request::Open(dialog, reply) => {
                    p.next += 1;
                    p.replies.insert(p.next, reply);
                    Effect::Open { id: p.next, dialog }
                }
                Request::Save(dialog, reply) => {
                    p.next += 1;
                    p.replies.insert(p.next, Box::new(move |paths: Vec<PathBuf>| reply(paths.into_iter().next())));
                    Effect::Save { id: p.next, dialog }
                }
                Request::Notify(n) => Effect::Notify(n),
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

    /// The system's answer to the dialog `Effect::Open` or `Effect::Save` asked for with `id`:
    /// the paths picked, none if it was cancelled. The app gets the message it asked for.
    pub fn answer(&mut self, id: u64, paths: Vec<PathBuf>) {
        if let Some(reply) = self.platform.replies.remove(&id) {
            self.send(reply(paths));
        }
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

    /// The app's menu-bar item as last sent.
    pub fn status_item(&self) -> Option<&MenuSpec> {
        self.platform.status.as_ref()
    }
}
