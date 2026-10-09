//! The operating system's appearance, light or dark, read once when the app starts, off the UI thread
//! (`main` runs [`mode`] on the runtime's blocking pool) through `orchester-exec`, which hides the
//! program's console. Windows keeps it in the registry (`AppsUseLightTheme`), read with
//! `%SystemRoot%\System32\reg.exe`; macOS in the global `AppleInterfaceStyle` default, read with
//! `/usr/bin/defaults`, which prints `Dark` in dark mode and fails when the key is absent (light
//! mode). Both programs ship with their system and are named by full path, so no program of the same
//! name elsewhere on the search path runs instead. Other platforms have no known source and say so.

use orchester_core::command::CommandError;

use crate::theme::Mode;

#[derive(Debug, thiserror::Error)]
pub enum SystemAppearanceError {
  #[error("SystemRoot is not set, so reg.exe cannot be found")]
  NoSystemRoot,
  #[error("cannot run {program}: {source}")]
  Run {
    program: &'static str,
    source: CommandError,
  },
  #[error("{program} did not report the appearance")]
  Unreadable { program: &'static str },
  #[error("the system appearance cannot be read on this platform yet")]
  Unsupported,
}

#[cfg(windows)]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  use std::path::PathBuf;

  const PROGRAM: &str = "reg.exe";
  let system_root = std::env::var_os("SystemRoot").ok_or(SystemAppearanceError::NoSystemRoot)?;
  let program = PathBuf::from(system_root).join("System32").join(PROGRAM);
  let output = run(
    &program,
    PROGRAM,
    [
      "query",
      r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
      "/v",
      "AppsUseLightTheme",
    ],
  )?;
  output
    .success()
    .then(|| windows_mode(&output.stdout))
    .flatten()
    .ok_or(SystemAppearanceError::Unreadable { program: PROGRAM })
}

#[cfg(target_os = "macos")]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  const PROGRAM: &str = "defaults";
  let output = run(
    std::path::Path::new("/usr/bin/defaults"),
    PROGRAM,
    ["read", "-g", "AppleInterfaceStyle"],
  )?;
  Ok(macos_mode(output.success(), &output.stdout))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn mode() -> Result<Mode, SystemAppearanceError> {
  Err(SystemAppearanceError::Unsupported)
}

#[cfg(any(windows, target_os = "macos"))]
fn run<const N: usize>(
  program: &std::path::Path,
  name: &'static str,
  args: [&str; N],
) -> Result<orchester_core::command::CommandOutput, SystemAppearanceError> {
  use orchester_core::command::{Command, CommandRunner};

  /// How long the program may take to answer.
  const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
  orchester_exec::ProcessRunner::new(TIMEOUT)
    .run(&Command::new(program, args))
    .map_err(|source| SystemAppearanceError::Run { program: name, source })
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
