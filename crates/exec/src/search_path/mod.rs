//! Where programs are looked up (`PATH`).
//!
//! An app started from Finder, the Dock or Spotlight on macOS inherits launchd's minimal `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), not the one the user's shell builds. The agent CLIs, `gh`,
//! Node.js (which npm-installed CLIs start through `#!/usr/bin/env node`) and the tools agents run
//! are usually elsewhere: Homebrew, npm, nvm, `~/.local/bin`. So on macOS the search path is the
//! inherited `PATH`, then the `PATH` the user's login shell builds (read once as the app starts,
//! [`read_login_shell`]), then the usual install folders that exist ([`macos`]). Connection
//! detection searches it, and every program Orchester starts gets it as its `PATH`, the runner
//! included, with the program's own folder last. Folders already on the `PATH` keep their place,
//! so a `PATH` from a terminal (`cargo run`) wins. Each folder records where it came from
//! ([`SearchPath`]), so a program that is not found names the folders searched.
//!
//! Windows programs inherit the user's full `PATH`, which is used as it is.

mod chosen;
mod folders;
pub mod login_shell;
#[cfg(target_os = "macos")]
pub mod macos;

use std::ffi::OsString;

pub use self::{
  chosen::{ChosenProgramError, chosen_program},
  folders::{FolderSource, ProgramLookup, SearchFolder, SearchPath},
};

/// The search path for programs: this process's `PATH`.
#[cfg(not(target_os = "macos"))]
pub fn search_path() -> Option<OsString> {
  std::env::var_os("PATH")
}

/// The search path for programs: this process's `PATH`, then the login shell's, then the usual
/// install folders.
#[cfg(target_os = "macos")]
pub fn search_path() -> Option<OsString> {
  Some(SearchPath::current().joined())
}

/// Nothing to read: Explorer gives every Windows app the user's full `PATH`, and there is no login
/// shell that builds another one.
#[cfg(not(target_os = "macos"))]
pub fn read_login_shell() {}

/// Starts reading the `PATH` the user's login shell builds, in the background, for the search path
/// of this process and the programs it starts. Called once, as the app starts; a process that does
/// not call it (the runner, which inherits the app's search path) never asks the shell.
#[cfg(target_os = "macos")]
pub fn read_login_shell() {
  macos::start_reading();
}
