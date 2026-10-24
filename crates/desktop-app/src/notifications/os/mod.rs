//! The operating system's side of the app's notifications (ORC-56): showing one, hearing its click, and the
//! *Needs you* count on the app's icon. Windows shows toasts under the app's AppUserModelID, which it registers for
//! the person (`HKCU\Software\Classes\AppUserModelId`), and puts an overlay with the count on the taskbar button;
//! macOS asks UserNotifications, which asks the person once for permission and works only for an app bundle, and
//! puts the count on the Dock icon. Either way a click hands its route to the window ([`Clicks`]), which opens it
//! and comes to the front. Only the installed app shows notifications; a development build or a QA copy stays silent
//! unless [`VARIABLE`] asks for them, so a QA run never notifies the person ([`Delivery`]). The badge is shown by
//! every build: it is that window's own icon.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use std::{ffi::OsString, sync::Arc};

use lurq::core::Signal;
use orchester_workspace::build::Build;
use parking_lot::Mutex;

/// `1` lets a build other than the installed app show OS notifications, under an id of its own.
pub const VARIABLE: &str = "ORCHESTER_APP_NOTIFICATIONS";

/// The id the installed app's notifications are shown under: its AppUserModelID on Windows.
const INSTALLED_ID: &str = "Orchester.App";
/// The id another build shows them under when [`VARIABLE`] asks, so they never mix with the installed app's.
const DEVELOPMENT_ID: &str = "Orchester.App.Development";

/// Whether this program shows OS notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
  /// Under this id.
  Shows(&'static str),
  /// A development build or a QA copy that was not asked to.
  Silent,
}

impl Delivery {
  /// [`Delivery::choose`] for this program and its environment.
  pub fn of_build() -> Self {
    Self::choose(Build::this(), std::env::var_os(VARIABLE))
  }

  /// The installed app shows them; another build only when `asked` is [`VARIABLE`]'s `1`.
  pub fn choose(build: Build, asked: Option<OsString>) -> Self {
    match build {
      Build::Installed => Self::Shows(INSTALLED_ID),
      Build::Other if asked.is_some_and(|value| value == "1") => Self::Shows(DEVELOPMENT_ID),
      Build::Other => Self::Silent,
    }
  }
}

/// A notification as the system shows it, in the person's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
  pub title: String,
  pub body: String,
  /// The route path a click opens.
  pub route: String,
  pub sound: bool,
}

/// Where the clicks of notifications arrive, from whichever thread the system reports them on: their routes, in
/// order, and a count the window renders on.
#[derive(Clone)]
pub struct Clicks {
  routes: Arc<Mutex<Vec<String>>>,
  count: Signal<u64>,
}

impl Clicks {
  pub fn new(count: Signal<u64>) -> Self {
    Self {
      routes: Arc::default(),
      count,
    }
  }

  /// A click of the notification that opens `route`.
  fn push(&self, route: String) {
    self.routes.lock().push(route);
    self.count.update(|count| *count += 1);
  }

  /// How many clicks came; reading it in a render renders again on the next.
  pub fn count(&self) -> u64 {
    self.count.get()
  }

  /// The routes clicked since the last take.
  pub fn take(&self) -> Vec<String> {
    std::mem::take(&mut *self.routes.lock())
  }
}

/// Why the system did not take a notification or the badge.
#[derive(Debug, thiserror::Error)]
pub enum OsError {
  #[cfg(windows)]
  #[error("Windows refused: {0}")]
  Windows(#[from] ::windows::core::Error),
  /// macOS shows notifications only for an app in a bundle (`Orchester.app`).
  #[error("this program does not run from an app bundle, which macOS notifications need")]
  NoAppBundle,
  #[error("the system's notification center is not available")]
  NoCenter,
  #[error("notifications are not supported on this platform yet")]
  Unsupported,
}

/// Sets up notifications under `id`, named `name` in the system's settings, with their clicks told to `clicks`; on
/// macOS this asks the person for permission the first time. Runs on the UI thread.
pub fn start(id: &'static str, name: &str, clicks: Clicks) -> Result<(), OsError> {
  #[cfg(windows)]
  return windows::toast::start(id, name, clicks);
  #[cfg(target_os = "macos")]
  return macos::start(id, name, clicks);
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    let _unused = (id, name, clicks);
    Err(OsError::Unsupported)
  }
}

/// Shows `shown`, once [`start`] set notifications up. Runs on the UI thread.
pub fn show(shown: &Shown) -> Result<(), OsError> {
  #[cfg(windows)]
  return windows::toast::show(shown);
  #[cfg(target_os = "macos")]
  return macos::show(shown);
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    let _unused = shown;
    Err(OsError::Unsupported)
  }
}

/// Shows `count` on the app's icon, none for 0; `label` says it in words where the system reads it out. `false` when
/// there is no icon to put it on yet (Windows, before the window has its taskbar button). Runs on the UI thread.
pub fn badge(count: usize, label: &str) -> Result<bool, OsError> {
  #[cfg(windows)]
  return windows::badge::set(count, label);
  #[cfg(target_os = "macos")]
  {
    let _unused = label;
    macos::badge(count).map(|()| true)
  }
  #[cfg(not(any(windows, target_os = "macos")))]
  {
    let _unused = (count, label);
    Err(OsError::Unsupported)
  }
}
