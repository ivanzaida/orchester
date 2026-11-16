//! Whether the `native` UI driver can work on this computer (`docs/architecture/agent-testing.md`, "UI drivers"), for
//! the app to show: on macOS it needs the Accessibility and Screen Recording permissions, and without one every
//! `ui_*` call of a service with that driver fails, so the person sees which is missing and where to grant it
//! ([`NativePermission::settings`], [`NativePermission::settings_url`]) before a run needs it. The runner asks the
//! same of its own process, whose permissions macOS takes from Orchester, the app that started it.

pub use orchester_exec::native_ui::{NativePermission, NativePermissions};

/// What the native driver can do on this computer now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeDriverState {
  /// It works: on Windows always, on macOS with both permissions granted.
  Ready,
  /// macOS has not granted Orchester these permissions, in [`NativePermission::ALL`]'s order.
  NeedsPermissions(Vec<NativePermission>),
  /// The platform has no native driver.
  NotSupported,
}

impl NativeDriverState {
  /// The state on this computer now. It asks macOS each time, since a person grants a permission while the app runs;
  /// it never prompts.
  pub fn now() -> Self {
    if !cfg!(any(windows, target_os = "macos")) {
      return Self::NotSupported;
    }
    Self::of(orchester_exec::native_ui::permissions())
  }

  /// The state of a platform where the driver exists, with `permissions`, or `None` where it needs none.
  pub fn of(permissions: Option<NativePermissions>) -> Self {
    let missing = permissions.map(NativePermissions::missing).unwrap_or_default();
    if missing.is_empty() {
      Self::Ready
    } else {
      Self::NeedsPermissions(missing)
    }
  }
}
