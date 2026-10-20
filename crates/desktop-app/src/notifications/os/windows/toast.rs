//! Toasts under the app's AppUserModelID. An app that is not packaged shows toasts only under an id registered for
//! the person: [`start`] writes its name under `HKCU\Software\Classes\AppUserModelId\<id>`, as the Windows App SDK
//! does for unpackaged apps. A click while the app runs reaches the toast's `Activated` handler, which hands its
//! route to the window; the toasts shown are kept alive for that. A toast clicked in the notification center after
//! the app closed opens nothing: that needs a COM activator registered with the system, which the app does not have.
//! The registry functions have no safe wrapper, so `unsafe` is allowed here and only for them.
#![allow(unsafe_code)]

use std::{collections::VecDeque, sync::OnceLock};

use parking_lot::Mutex;
use windows::{
  Data::Xml::Dom::XmlDocument,
  Foundation::TypedEventHandler,
  UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier},
  Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, REG_SZ, RegCloseKey, RegCreateKeyW, RegSetValueExW},
  core::{HSTRING, w},
};

use super::super::{Clicks, OsError, Shown};

/// How many toasts are kept to hear their clicks; older ones have left the screen for the notification center.
const KEPT: usize = 32;

struct Toasts {
  notifier: ToastNotifier,
  clicks: Clicks,
  shown: Mutex<VecDeque<ToastNotification>>,
}

static TOASTS: OnceLock<Toasts> = OnceLock::new();

pub fn start(id: &'static str, name: &str, clicks: Clicks) -> Result<(), OsError> {
  register(id, name)?;
  let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(id))?;
  // A second start (another window of the same process) keeps the first's notifier and clicks.
  if TOASTS
    .set(Toasts {
      notifier,
      clicks,
      shown: Mutex::new(VecDeque::new()),
    })
    .is_err()
  {
    tracing::debug!("notifications were set up already");
  }
  Ok(())
}

pub fn show(shown: &Shown) -> Result<(), OsError> {
  let toasts = TOASTS.get().ok_or(OsError::NoCenter)?;
  let document = XmlDocument::new()?;
  document.LoadXml(&HSTRING::from(content(shown)))?;
  let toast = ToastNotification::CreateToastNotification(&document)?;
  let (clicks, route) = (toasts.clicks.clone(), shown.route.clone());
  toast.Activated(&TypedEventHandler::new(move |_, _| {
    clicks.push(route.clone());
    Ok(())
  }))?;
  toasts.notifier.Show(&toast)?;
  let mut kept = toasts.shown.lock();
  kept.push_back(toast);
  if kept.len() > KEPT {
    kept.pop_front();
  }
  Ok(())
}

/// The toast's XML: its two lines, the route as its launch argument, and no sound when `shown` has none.
fn content(shown: &Shown) -> String {
  let audio = if shown.sound { "" } else { r#"<audio silent="true"/>"# };
  format!(
    r#"<toast launch="{}" activationType="foreground"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{audio}</toast>"#,
    escape(&shown.route),
    escape(&shown.title),
    escape(&shown.body),
  )
}

fn escape(text: &str) -> String {
  text
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&apos;")
}

/// Registers `id` for the person, named `name` where Windows lists the apps that notify.
fn register(id: &str, name: &str) -> Result<(), OsError> {
  let path = HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{id}"));
  let mut key = HKEY::default();
  // SAFETY: `path` is a live, null-terminated wide string and `key` a live handle the call writes.
  unsafe { RegCreateKeyW(HKEY_CURRENT_USER, &path, &raw mut key) }.ok()?;
  let value: Vec<u8> = name.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
  // SAFETY: `key` was opened above; the value is a null-terminated UTF-16 string, as REG_SZ asks.
  let written = unsafe { RegSetValueExW(key, w!("DisplayName"), None, REG_SZ, Some(&value)) }.ok();
  // SAFETY: `key` was opened above and is closed once.
  let closed = unsafe { RegCloseKey(key) }.ok();
  written?;
  closed?;
  Ok(())
}
