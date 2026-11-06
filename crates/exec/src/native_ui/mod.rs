//! The `native` UI driver (`docs/architecture/agent-testing.md`, "UI drivers"): the windows of a program the run
//! started, seen and used through the operating system's accessibility interface, never through the computer's input.
//!
//! - Windows ([`windows`]): the visible top-level windows of the processes of the service's tree that pass the run's
//!   check ([`crate::run_processes::RunTree`]); each is moved off-screen without being activated when it is opened. A
//!   snapshot is the UI Automation tree of each window (its control view), each element with a ref minted from its
//!   runtime id, its control type, its name, its states and its value. Actions use element patterns only — Invoke,
//!   Toggle, SelectionItem, ExpandCollapse, Value, ScrollItem — and an element without the needed one answers
//!   *unsupported for this element*; no key is pressed and no point is clicked. A screenshot is the window's own
//!   rendering (`PrintWindow`) into memory, never the screen.
//! - macOS ([`macos`]): the normal-layer windows of those processes as the window server lists them, each matched to
//!   its Accessibility (AX) window. A snapshot is the AXUIElement tree of each, mapped by [`accessibility`]; actions
//!   are AX actions and settable attributes only (`AXPress`, `AXPick`, `AXValue`, `AXIncrement`/`AXDecrement`,
//!   `AXScrollToVisible`), never CGEvent input. A screenshot is that window alone by its window id, through
//!   ScreenCaptureKit. Every call first needs the Accessibility and Screen Recording permissions ([`permission`]) and
//!   answers which are missing and where to grant them.
//! - Other platforms ([`unsupported`]): every call answers that the driver is not supported on this platform.

pub mod accessibility;
#[cfg(any(windows, target_os = "macos"))]
mod crop;
#[cfg(target_os = "macos")]
mod macos;
pub mod permission;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod windows;

use orchester_core::orchester_tools::ui::UiElement;
use thiserror::Error;

#[cfg(target_os = "macos")]
pub use self::macos::{act, capture, find_windows, idle, open, permissions, snapshot};
pub use self::permission::{NativePermission, NativePermissions};
#[cfg(not(any(windows, target_os = "macos")))]
pub use self::unsupported::{act, capture, find_windows, idle, open, snapshot};
#[cfg(windows)]
pub use self::windows::{act, capture, find_windows, idle, open, snapshot};

/// The permissions the driver needs of this computer: none outside macOS.
#[cfg(not(target_os = "macos"))]
pub fn permissions() -> Option<NativePermissions> {
  None
}

/// Nothing when the driver can work on this computer now; else why not — on macOS the permissions it misses. Every
/// `ui_*` call of a service with the native driver asks this first, so each one names a missing permission.
pub fn ready() -> Result<(), NativeUiError> {
  if !cfg!(any(windows, target_os = "macos")) {
    return Err(NativeUiError::NotSupported);
  }
  permissions().map_or(Ok(()), NativePermissions::require)
}

/// The most elements one snapshot answers.
pub const SNAPSHOT_NODES: usize = 300;
/// How deep one snapshot looks.
pub const SNAPSHOT_DEPTH: usize = 40;
/// The most elements a query or an action looks through before it gives up.
pub const SEARCH_NODES: usize = 5000;

/// A top-level window of a process of the run, as the driver found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeWindow {
  /// Its handle, valid while its process runs.
  #[cfg(windows)]
  handle: isize,
  /// Its window server id (`CGWindowID`), valid while the window lives.
  #[cfg(target_os = "macos")]
  window_id: u32,
  pub pid: u32,
  pub title: String,
  pub bounds: Bounds,
  /// Whether it lies outside every monitor of the computer, where no person sees it.
  pub off_screen: bool,
}

/// A rectangle on the computer's desktop: in pixels on Windows, in points on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32,
}

/// What a snapshot holds: the tree, or the elements a query matched, without their children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeSnapshot {
  Tree { tree: UiElement, truncated: bool },
  Matches { matches: Vec<UiElement>, truncated: bool },
}

/// A capture of a window, or of one element of it: its pixels, row by row from the top, 4 bytes (RGBA) each.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeCapture {
  pub width: u32,
  pub height: u32,
  pub rgba: Vec<u8>,
}

/// Why a call of the native driver did not do what it asked.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NativeUiError {
  #[error(
    "the native UI driver is not supported on this platform: it works on Windows through UI Automation and on macOS \
     through Accessibility"
  )]
  NotSupported,
  /// macOS has not granted Orchester these permissions; each is named with where a person grants it.
  #[error("{}", permission::describe(.0))]
  PermissionsMissing(Vec<NativePermission>),
  #[error("the service shows no window of a process this run started")]
  NoWindow,
  #[error("the service's windows cannot be listed or placed: {0}")]
  Windows(String),
  #[error("no element has the ref `{0}` now: take a new ui_snapshot")]
  NoElement(String),
  #[error(
    "unsupported for this element: {action} needs {needs}, and the element `{reference}` ({role}) has none of them; \
     the native driver never clicks a point or presses keys instead"
  )]
  Unsupported {
    action: &'static str,
    /// The patterns (Windows) or AX actions and attributes (macOS) that would do it.
    needs: &'static str,
    reference: String,
    role: String,
  },
  #[error("the element `{reference}` holds no option named `{option}`")]
  NoOption { reference: String, option: String },
  #[error("the element `{0}` is read-only")]
  ReadOnly(String),
  #[error(
    "the native driver presses no keys: it acts only through the elements' own accessibility actions (click, type, \
     scroll, select)"
  )]
  NoKeys,
  #[error("the window did not become idle within {0} ms")]
  Busy(u32),
  #[error("UI Automation failed: {0}")]
  Automation(String),
  #[error("macOS Accessibility failed: {0}")]
  Accessibility(String),
  #[error("the window cannot be captured: {0}")]
  Capture(String),
}
