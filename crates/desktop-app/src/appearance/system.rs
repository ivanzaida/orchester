//! The operating system's appearance, light or dark. Windows keeps it in the registry
//! (`AppsUseLightTheme`), read with `reg query`; macOS in the global `AppleInterfaceStyle` default, read
//! with `defaults read`, which prints `Dark` in dark mode and fails when the key is absent (light mode).
//! Both programs ship with their system. Other platforms have no known source and say so.

use crate::theme::Mode;

#[derive(Debug, thiserror::Error)]
pub enum SystemAppearanceError {
  #[error("cannot run {program}: {source}")]
  Run {
    program: &'static str,
    source: std::io::Error,
  },
  #[error("{program} did not report the appearance")]
  Unreadable { program: &'static str },
  #[error("the system appearance cannot be read on this platform yet")]
  Unsupported,
}

#[cfg(windows)]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  use std::os::windows::process::CommandExt;

  const PROGRAM: &str = "reg";
  /// No console window opens for the program (`CREATE_NO_WINDOW`).
  const CREATE_NO_WINDOW: u32 = 0x0800_0000;
  let output = std::process::Command::new(PROGRAM)
    .args([
      "query",
      r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
      "/v",
      "AppsUseLightTheme",
    ])
    .creation_flags(CREATE_NO_WINDOW)
    .output()
    .map_err(|source| SystemAppearanceError::Run {
      program: PROGRAM,
      source,
    })?;
  output
    .status
    .success()
    .then(|| windows_mode(&String::from_utf8_lossy(&output.stdout)))
    .flatten()
    .ok_or(SystemAppearanceError::Unreadable { program: PROGRAM })
}

#[cfg(target_os = "macos")]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  const PROGRAM: &str = "defaults";
  let output = std::process::Command::new(PROGRAM)
    .args(["read", "-g", "AppleInterfaceStyle"])
    .output()
    .map_err(|source| SystemAppearanceError::Run {
      program: PROGRAM,
      source,
    })?;
  Ok(macos_mode(
    output.status.success(),
    &String::from_utf8_lossy(&output.stdout),
  ))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  Err(SystemAppearanceError::Unsupported)
}

/// The mode in `reg query`'s answer for `AppsUseLightTheme`: `0x0` is dark, `0x1` light.
pub fn windows_mode(answer: &str) -> Option<Mode> {
  let line = answer.lines().find(|line| line.contains("AppsUseLightTheme"))?;
  let value = line.split_whitespace().last()?.strip_prefix("0x")?;
  match u32::from_str_radix(value, 16).ok()? {
    0 => Some(Mode::Dark),
    _ => Some(Mode::Light),
  }
}

/// The mode `defaults read -g AppleInterfaceStyle` reports: `Dark` when it succeeds with it, light when
/// the key is absent and the program fails.
pub fn macos_mode(succeeded: bool, answer: &str) -> Mode {
  if succeeded && answer.trim().eq_ignore_ascii_case("dark") {
    Mode::Dark
  } else {
    Mode::Light
  }
}
