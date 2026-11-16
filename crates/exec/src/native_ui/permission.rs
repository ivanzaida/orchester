//! The macOS permissions the native driver needs (`docs/architecture/agent-testing.md`, "UI drivers"): Accessibility
//! to read and use a window's elements, Screen Recording to capture the window. A person grants both to Orchester in
//! System Settings; the driver only looks whether they are granted and never asks for them, so no system prompt
//! appears during a run.

use super::NativeUiError;

/// A permission of macOS the native driver needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativePermission {
  /// Reading and using other apps' elements (`AXIsProcessTrusted`).
  Accessibility,
  /// Capturing other apps' windows (`CGPreflightScreenCaptureAccess`).
  ScreenRecording,
}

impl NativePermission {
  /// Every one, in the order they are named.
  pub const ALL: [Self; 2] = [Self::Accessibility, Self::ScreenRecording];

  /// Its name in System Settings.
  pub fn name(self) -> &'static str {
    match self {
      Self::Accessibility => "Accessibility",
      Self::ScreenRecording => "Screen Recording",
    }
  }

  /// Where in System Settings a person grants it.
  pub fn settings(self) -> &'static str {
    match self {
      Self::Accessibility => "System Settings › Privacy & Security › Accessibility",
      Self::ScreenRecording => "System Settings › Privacy & Security › Screen Recording",
    }
  }

  /// The link that opens that pane of System Settings.
  pub fn settings_url(self) -> &'static str {
    match self {
      Self::Accessibility => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
      Self::ScreenRecording => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
    }
  }
}

/// Which of the permissions macOS grants Orchester now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePermissions {
  pub accessibility: bool,
  pub screen_recording: bool,
}

impl NativePermissions {
  /// Whether `permission` is granted.
  pub fn granted(self, permission: NativePermission) -> bool {
    match permission {
      NativePermission::Accessibility => self.accessibility,
      NativePermission::ScreenRecording => self.screen_recording,
    }
  }

  /// The permissions not granted, in [`NativePermission::ALL`]'s order.
  pub fn missing(self) -> Vec<NativePermission> {
    NativePermission::ALL
      .into_iter()
      .filter(|permission| !self.granted(*permission))
      .collect()
  }

  /// Nothing when every permission is granted; else the error that names each missing one and where to grant it.
  pub fn require(self) -> Result<(), NativeUiError> {
    let missing = self.missing();
    if missing.is_empty() {
      Ok(())
    } else {
      Err(NativeUiError::PermissionsMissing(missing))
    }
  }
}

/// What [`NativeUiError::PermissionsMissing`] says of `missing`.
pub(super) fn describe(missing: &[NativePermission]) -> String {
  let names: Vec<&str> = missing.iter().map(|permission| permission.name()).collect();
  let grants: Vec<String> = missing
    .iter()
    .map(|permission| format!("grant {} in {}", permission.name(), permission.settings()))
    .collect();
  format!(
    "permission missing: macOS has not granted Orchester the {} permission{}, which the native UI driver needs; {}, \
     then restart Orchester and call again",
    names.join(" and "),
    if missing.len() == 1 { "" } else { "s" },
    grants.join("; ")
  )
}
