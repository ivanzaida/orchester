//! The command that installs git on this platform (`L7JC5M` in `KsWqe`): winget on Windows, Apple's command line
//! tools on macOS. Orchester never runs it. Another platform has none, and the block says to use its package
//! manager.

#[cfg(target_os = "windows")]
pub(super) const INSTALL_GIT: Option<&str> = Some("winget install --id Git.Git -e --source winget");

#[cfg(target_os = "macos")]
pub(super) const INSTALL_GIT: Option<&str> = Some("xcode-select --install");

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) const INSTALL_GIT: Option<&str> = None;
