//! Whether the operating system asks apps to keep still (reduced motion), read once when the app starts, off the UI
//! thread (`window::open` runs [`reduced`] on the runtime's blocking pool), as the system appearance is. Only what
//! merely decorates follows it, such as a skeleton's pulse; a loader still turns, since it says work goes on.
//!
//! Windows keeps it as *Animation effects* (Settings › Accessibility › Visual effects), which
//! `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` reads ([`windows`]); macOS as *Reduce motion* (Accessibility ›
//! Display), the `reduceMotion` default of `com.apple.universalaccess`, read with `/usr/bin/defaults`, which prints
//! `1` while it is on and fails while the key was never set (off). Other platforms have no known source and say so.

#[cfg(windows)]
mod windows;

#[derive(Debug, thiserror::Error)]
pub enum MotionError {
  #[error("Windows did not say whether animation effects are on")]
  Unreadable,
  #[error("cannot run {program}: {source}")]
  Run {
    program: &'static str,
    source: orchester_core::command::CommandError,
  },
  #[error("the reduced motion setting cannot be read on this platform yet")]
  Unsupported,
}

/// Whether the person turned animations off in the operating system.
#[cfg(windows)]
pub fn reduced() -> Result<bool, MotionError> {
  windows::animations_on().map(|on| !on)
}

#[cfg(target_os = "macos")]
pub fn reduced() -> Result<bool, MotionError> {
  use orchester_core::command::{Command, CommandRunner};

  const PROGRAM: &str = "defaults";
  /// How long the program may take to answer.
  const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
  let output = orchester_exec::ProcessRunner::new(TIMEOUT)
    .run(&Command::new(
      std::path::Path::new("/usr/bin/defaults"),
      ["read", "com.apple.universalaccess", "reduceMotion"],
    ))
    .map_err(|source| MotionError::Run {
      program: PROGRAM,
      source,
    })?;
  Ok(macos_reduced(output.success(), &output.stdout))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn reduced() -> Result<bool, MotionError> {
  Err(MotionError::Unsupported)
}

/// What `defaults read com.apple.universalaccess reduceMotion` says: on when it succeeds with `1`, off when the key was
/// never set and the program fails.
pub fn macos_reduced(succeeded: bool, answer: &str) -> bool {
  succeeded && answer.trim() == "1"
}
