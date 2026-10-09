//! *Switch to that window* on the *already open* screen (`N0Lpc`): bringing the window of the app that
//! has the data folder open to the front. Windows lets the app the person is using hand the foreground
//! to another process's window. macOS has no way to do that without Accessibility permissions, so it is
//! not offered there yet and the screen says so.

#[cfg(target_os = "windows")]
mod windows;

use orchester_store::location::LockHolder;

/// What asking another app's window to come to the front did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Front {
  /// Its window is in front now.
  Brought,
  /// It has no window, it is not responding, or Windows refused: it stays where it is.
  NoAnswer,
}

/// Whether this platform can bring another app's window to the front.
pub const SUPPORTED: bool = cfg!(target_os = "windows");

/// Brings the main window of the app `holder` names to the front, restoring it when it is minimized,
/// once its process is known to be that Orchester app: an Orchester program created before its session
/// started, so a process id reused since is never brought forward. lurq's DevTools window is passed over.
#[cfg(target_os = "windows")]
pub fn bring_to_front(holder: LockHolder) -> Front {
  windows::bring_to_front(holder.pid, holder.started_at)
}

/// Not offered where [`SUPPORTED`] is false; asking anyway gets no answer.
#[cfg(not(target_os = "windows"))]
pub fn bring_to_front(_holder: LockHolder) -> Front {
  Front::NoAnswer
}
