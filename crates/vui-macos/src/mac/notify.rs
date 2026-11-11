//! Notifications through `UNUserNotificationCenter`, with buttons whose answers come back to the
//! window that asked, as `Ui::answer`.
//!
//! The centre only works for an app that runs from a bundle (`.app`) with an identifier; anything
//! else, `cargo run` included, gets a warning on stderr instead of a notification. Every request is
//! named `vui.<process>.<window>.<id>`, so an answer finds the window and the request it belongs to,
//! and answers to an earlier run's notifications are ignored. Each set of button titles gets a
//! category of its own, registered the first time it is used; every category asks to hear about
//! dismissals.

use std::collections::HashSet;

use objc2::runtime::Bool;
use objc2_foundation::{NSBundle, NSError, NSSet};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationAction, UNNotificationActionOptions,
    UNNotificationCategory, UNNotificationCategoryOptions, UNNotificationDefaultActionIdentifier, UNNotificationDismissActionIdentifier,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use vui_runtime::{Notification, NotificationResponse};

use super::*;

/// What the main thread keeps for the notification centre.
#[derive(Default)]
struct Centre {
    /// The centre holds its delegate weakly.
    delegate: Option<Retained<NotificationDelegate>>,
    /// Categories registered so far: setting them replaces the whole set.
    categories: Vec<Retained<UNNotificationCategory>>,
    names: HashSet<String>,
    warned: bool,
}

thread_local! {
    static CENTRE: RefCell<Centre> = RefCell::new(Centre::default());
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate has no state or Drop.
    #[unsafe(super(NSObject))]
    #[name = "VuiNotificationDelegate"]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        /// Shows the banner even while the app is in front, which the centre does not by default.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _centre: &UNUserNotificationCenter,
            _notification: &UNNotification,
            done: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let show = UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List | UNNotificationPresentationOptions::Sound;
            done.call((show,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(&self, _centre: &UNUserNotificationCenter, response: &UNNotificationResponse, done: &block2::DynBlock<dyn Fn()>) {
            let request = response.notification().request().identifier().to_string();
            let action = response.actionIdentifier();
            // SAFETY: both are constant strings the framework defines.
            let answer = if action.isEqualToString(unsafe { UNNotificationDefaultActionIdentifier }) {
                Some(NotificationResponse::Clicked)
            } else if action.isEqualToString(unsafe { UNNotificationDismissActionIdentifier }) {
                Some(NotificationResponse::Dismissed)
            } else {
                parse_action(&action.to_string()).map(NotificationResponse::Action)
            };
            if let (Some((window, id)), Some(answer)) = (parse_request(&request, std::process::id()), answer) {
                // The centre may call from any queue; windows live on the main thread.
                DispatchQueue::main().exec_async(move || answer_window(window, id, answer));
            }
            done.call(());
        }
    }
);

fn answer_window(window: usize, id: u64, answer: NotificationResponse) {
    if let Some(view) = VIEWS.with(|views| views.borrow().get(window).and_then(Weak::load)) {
        let effects = view.with_host(|h| h.answer(id, answer.into())).unwrap_or_default();
        view.after_input(effects);
    }
}

/// Whether the process runs from an app bundle with an identifier: the notification centre throws
/// otherwise.
fn bundled() -> bool {
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier().is_some() && bundle.bundlePath().to_string().ends_with(".app")
}

/// Sets the delegate, so answers reach the app; called at launch, so an answer that launched the
/// app is heard too.
pub(super) fn install() {
    if !bundled() {
        return;
    }
    CENTRE.with(|c| {
        let mut c = c.borrow_mut();
        if c.delegate.is_none() {
            let delegate: Retained<NotificationDelegate> = unsafe { msg_send![NotificationDelegate::alloc(), init] };
            UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            c.delegate = Some(delegate);
        }
    });
}

impl VuiView {
    /// This view's number among the app's windows, as its `Proxy` knows it.
    fn index(&self) -> Option<usize> {
        VIEWS.with(|views| views.borrow().iter().position(|w| w.load().is_some_and(|v| std::ptr::eq(&*v, self))))
    }

    pub(super) fn notify(&self, id: u64, n: &Notification) {
        let Some(window) = self.index() else { return };
        if !bundled() {
            CENTRE.with(|c| {
                if !std::mem::replace(&mut c.borrow_mut().warned, true) {
                    eprintln!("vui: notifications need the app to run from an app bundle; {:?} was not shown", n.title);
                }
            });
            return;
        }
        install();
        let centre = UNUserNotificationCenter::currentNotificationCenter();
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&n.title));
        content.setBody(&NSString::from_str(&n.body));
        if n.sound {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        content.setCategoryIdentifier(&NSString::from_str(&category(&centre, &n.actions)));
        let name = NSString::from_str(&request_id(std::process::id(), window, id));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&name, &content, None);
        // Asking again costs nothing once the person has decided; the system asks them only once.
        let poster = centre.clone();
        let post = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            if granted.as_bool() {
                poster.addNotificationRequest_withCompletionHandler(&request, None);
            }
        });
        centre.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &post);
    }
}

/// The category for a set of button titles, registered on first use.
fn category(centre: &UNUserNotificationCenter, actions: &[String]) -> String {
    let name = category_name(actions);
    CENTRE.with(|c| {
        let mut c = c.borrow_mut();
        if c.names.insert(name.clone()) {
            let buttons: Vec<Retained<UNNotificationAction>> = actions
                .iter()
                .enumerate()
                .map(|(i, title)| {
                    UNNotificationAction::actionWithIdentifier_title_options(
                        &NSString::from_str(&action_id(i)),
                        &NSString::from_str(title),
                        UNNotificationActionOptions::empty(),
                    )
                })
                .collect();
            let category = UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                &NSString::from_str(&name),
                &NSArray::from_retained_slice(&buttons),
                &NSArray::new(),
                UNNotificationCategoryOptions::CustomDismissAction,
            );
            c.categories.push(category);
            centre.setNotificationCategories(&NSSet::from_retained_slice(&c.categories));
        }
    });
    name
}

fn request_id(process: u32, window: usize, id: u64) -> String {
    format!("vui.{process}.{window}.{id}")
}

/// The window and request a request name belongs to, if this process sent it.
fn parse_request(name: &str, process: u32) -> Option<(usize, u64)> {
    let mut parts = name.strip_prefix("vui.")?.split('.');
    let (p, w, id) = (parts.next()?, parts.next()?, parts.next()?);
    (parts.next().is_none() && p.parse() == Ok(process)).then_some(())?;
    Some((w.parse().ok()?, id.parse().ok()?))
}

fn action_id(i: usize) -> String {
    format!("vui.action.{i}")
}

fn parse_action(name: &str) -> Option<usize> {
    name.strip_prefix("vui.action.")?.parse().ok()
}

/// A name for a set of button titles that stays the same from run to run (FNV-1a).
fn category_name(actions: &[String]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in actions.iter().flat_map(|a| a.bytes().chain([0])) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    format!("vui.category.{}.{hash:016x}", actions.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_names_round_trip_and_belong_to_one_process() {
        let name = request_id(42, 3, 17);
        assert_eq!(parse_request(&name, 42), Some((3, 17)));
        assert_eq!(parse_request(&name, 43), None, "an earlier run's notification");
        assert_eq!(parse_request("vui.42.3", 42), None);
        assert_eq!(parse_request("vui.42.3.17.1", 42), None);
        assert_eq!(parse_request("other.42.3.17", 42), None);
    }

    #[test]
    fn action_names_round_trip() {
        assert_eq!(parse_action(&action_id(2)), Some(2));
        assert_eq!(parse_action("com.apple.UNNotificationDefaultActionIdentifier"), None);
    }

    #[test]
    fn categories_are_named_by_their_buttons() {
        let a = category_name(&["Open".into(), "Retry".into()]);
        assert_eq!(a, category_name(&["Open".into(), "Retry".into()]));
        assert_ne!(a, category_name(&["Retry".into(), "Open".into()]));
        assert_ne!(category_name(&["ab".into(), "c".into()]), category_name(&["a".into(), "bc".into()]));
        assert_ne!(category_name(&[]), a);
    }
}
