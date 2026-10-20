//! macOS notifications through UserNotifications, and the *Needs you* count on the Dock icon. UserNotifications
//! serves only an app in a bundle with a bundle identifier (`Orchester App.app`, `dev.orchester.app`): for a program
//! run outside one (`cargo run`) its notification center raises an exception, so [`start`] refuses that case and the
//! app shows none, saying so in its log. The first start asks the person for permission; until they allow it the
//! system drops what is shown, and an ad-hoc signed, not notarized bundle may be refused by the system altogether.
//! A click activates the app and hands its route to the window through the center's delegate. The id is the
//! bundle's: UserNotifications takes no other. The bindings of UserNotifications for this objc2 are not in the build,
//! so it is reached through the Objective-C runtime; that is FFI without a safe wrapper, so `unsafe` is allowed here
//! and only here.
#![allow(unsafe_code)]

use std::cell::OnceCell;

use block2::{Block, RcBlock};
use objc2::{
  DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
  rc::Retained,
  runtime::{AnyClass, AnyObject, Bool, NSObject, NSObjectProtocol},
};
use objc2_foundation::NSString;

use super::{Clicks, OsError, Shown};

#[link(name = "UserNotifications", kind = "framework")]
unsafe extern "C" {}

/// `UNAuthorizationOptionBadge | UNAuthorizationOptionSound | UNAuthorizationOptionAlert`.
const AUTHORIZATION: usize = 1 | 2 | 4;
/// `UNNotificationPresentationOptionSound | List | Banner`: the app decides itself when a notification is of use,
/// so one it shows while active is shown as when it is not.
const PRESENTATION: usize = (1 << 1) | (1 << 3) | (1 << 4);
/// The key of a notification's `userInfo` that holds the route a click opens.
const ROUTE: &str = "route";

define_class!(
  /// Hears the clicks of the app's notifications.
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[name = "OrchesterNotificationDelegate"]
  #[ivars = Clicks]
  struct Delegate;

  unsafe impl NSObjectProtocol for Delegate {}

  impl Delegate {
    #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
    fn received(&self, _center: &AnyObject, response: &AnyObject, done: &Block<dyn Fn()>) {
      if let Some(route) = route_of(response) {
        self.ivars().push(route);
      }
      activate();
      done.call(());
    }

    #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
    fn presents(&self, _center: &AnyObject, _notification: &AnyObject, done: &Block<dyn Fn(usize)>) {
      done.call((PRESENTATION,));
    }
  }
);

/// The center and its delegate, which the center holds weakly; on the UI thread only.
struct Center {
  center: Retained<AnyObject>,
  _delegate: Retained<Delegate>,
}

thread_local! {
  static CENTER: OnceCell<Center> = const { OnceCell::new() };
}

fn class(name: &std::ffi::CStr) -> Result<&'static AnyClass, OsError> {
  AnyClass::get(name).ok_or(OsError::NoCenter)
}

pub fn start(_id: &'static str, _name: &str, clicks: Clicks) -> Result<(), OsError> {
  let program = std::env::current_exe().map_err(|_| OsError::NoAppBundle)?;
  if !orchester_store::location::macos::in_app_bundle(&program) {
    return Err(OsError::NoAppBundle);
  }
  let main = MainThreadMarker::new().ok_or(OsError::NoCenter)?;
  let center_class = class(c"UNUserNotificationCenter")?;
  // SAFETY: a class method without arguments that returns the app's center, retained by `Retained`.
  let center: Retained<AnyObject> = unsafe { msg_send![center_class, currentNotificationCenter] };
  let delegate = main.alloc::<Delegate>().set_ivars(clicks);
  // SAFETY: `init` of NSObject, on a freshly allocated instance.
  let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
  // SAFETY: the delegate answers the protocol's methods it is asked for; the center keeps it weakly, `Center` keeps it.
  let () = unsafe { msg_send![&*center, setDelegate: &*delegate] };
  let answered = RcBlock::new(|granted: Bool, _error: *mut AnyObject| {
    tracing::info!(granted = granted.as_bool(), "macOS answered whether the app may notify");
  });
  // SAFETY: the options are UNAuthorizationOptions; the block lives as long as the center needs it (it copies it).
  let () =
    unsafe { msg_send![&*center, requestAuthorizationWithOptions: AUTHORIZATION, completionHandler: &*answered] };
  CENTER.with(|cell| {
    if cell
      .set(Center {
        center,
        _delegate: delegate,
      })
      .is_err()
    {
      tracing::debug!("notifications were set up already");
    }
  });
  Ok(())
}

pub fn show(shown: &Shown) -> Result<(), OsError> {
  let content_class = class(c"UNMutableNotificationContent")?;
  let request_class = class(c"UNNotificationRequest")?;
  let dictionary_class = class(c"NSDictionary")?;
  let sound_class = class(c"UNNotificationSound")?;
  CENTER.with(|cell| {
    let center = cell.get().ok_or(OsError::NoCenter)?;
    // SAFETY: each message is one UserNotifications and Foundation document, with arguments of the types they take;
    // every object is retained by `Retained` for as long as it is used.
    unsafe {
      let content: Retained<AnyObject> = msg_send![content_class, new];
      let () = msg_send![&*content, setTitle: &*NSString::from_str(&shown.title)];
      let () = msg_send![&*content, setBody: &*NSString::from_str(&shown.body)];
      if shown.sound {
        let sound: Retained<AnyObject> = msg_send![sound_class, defaultSound];
        let () = msg_send![&*content, setSound: &*sound];
      }
      let info: Retained<AnyObject> = msg_send![
        dictionary_class,
        dictionaryWithObject: &*NSString::from_str(&shown.route),
        forKey: &*NSString::from_str(ROUTE)
      ];
      let () = msg_send![&*content, setUserInfo: &*info];
      let id = NSString::from_str(&uuid::Uuid::new_v4().to_string());
      let request: Retained<AnyObject> = msg_send![
        request_class,
        requestWithIdentifier: &*id,
        content: &*content,
        trigger: Option::<&AnyObject>::None
      ];
      let () = msg_send![
        &*center.center,
        addNotificationRequest: &*request,
        withCompletionHandler: Option::<&Block<dyn Fn(*mut AnyObject)>>::None
      ];
    }
    Ok(())
  })
}

pub fn badge(count: usize) -> Result<(), OsError> {
  MainThreadMarker::new().ok_or(OsError::NoCenter)?;
  let application = class(c"NSApplication")?;
  let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
  // SAFETY: on the main thread, as AppKit asks; the label is an NSString or nil, which clears the badge.
  unsafe {
    let app: Retained<AnyObject> = msg_send![application, sharedApplication];
    let tile: Retained<AnyObject> = msg_send![&*app, dockTile];
    let () = msg_send![&*tile, setBadgeLabel: label.as_deref()];
  }
  Ok(())
}

/// Brings the app to the front, on a click of its notification.
fn activate() {
  let Ok(application) = class(c"NSApplication") else {
    return;
  };
  // SAFETY: on the main thread, where the center calls its delegate.
  unsafe {
    let app: Retained<AnyObject> = msg_send![application, sharedApplication];
    let () = msg_send![&*app, activateIgnoringOtherApps: Bool::YES];
  }
}

/// The route the clicked notification of `response` carries.
fn route_of(response: &AnyObject) -> Option<String> {
  // SAFETY: the properties of UNNotificationResponse, UNNotification, UNNotificationRequest and its content, each an
  // object or nil; the route is the NSString `show` put there.
  unsafe {
    let notification: Option<Retained<AnyObject>> = msg_send![response, notification];
    let request: Option<Retained<AnyObject>> = msg_send![&*notification?, request];
    let content: Option<Retained<AnyObject>> = msg_send![&*request?, content];
    let info: Option<Retained<AnyObject>> = msg_send![&*content?, userInfo];
    let route: Option<Retained<NSString>> = msg_send![&*info?, objectForKey: &*NSString::from_str(ROUTE)];
    route.map(|route| route.to_string())
  }
}
