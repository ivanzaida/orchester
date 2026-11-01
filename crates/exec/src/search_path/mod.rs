//! Where programs are looked up (`PATH`).
//!
//! An app started from Finder, the Dock or Spotlight on macOS inherits launchd's minimal `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), not the one the user's shell builds. The agent CLIs, `gh`,
//! Node.js (which npm-installed CLIs start through `#!/usr/bin/env node`) and the tools agents run
//! are usually elsewhere: Homebrew, npm, nvm, `~/.local/bin`. So on macOS the search path is the
//! inherited `PATH`, then the `PATH` the user's login shell builds (read as the app starts,
//! [`read_login_shell`], and again when a person asks, [`read_login_shell_again`]), then the usual
//! install folders that exist ([`macos`]). Connection detection searches it, and every program
//! Orchester starts gets it as its `PATH`, the runner included, with the program's own folder last.
//! Folders already on the `PATH` keep their place, so a `PATH` from a terminal (`cargo run`) wins.
//! Each folder records where it came from ([`SearchPath`]), so a program that is not found names
//! the folders searched, and [`search_basis`] says what the search can claim it looked at.
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
  folders::{FolderSource, ProgramLookup, SearchBasis, SearchFolder, SearchPath, shown_folder},
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

/// Nothing to read, as for [`read_login_shell`]: Explorer gives every Windows app the user's full `PATH`, and there is
/// no login shell that builds another one.
#[cfg(not(target_os = "macos"))]
pub fn read_login_shell_again() {}

/// Reads the login shell's `PATH` again, as a person's *Check again* asks: their shell's start-up files may have
/// changed. Blocks for at most [`login_shell::TIMEOUT`]; a process that does not read the shell never asks it.
#[cfg(target_os = "macos")]
pub fn read_login_shell_again() {
  macos::read_again();
}

/// What a search of the search path looks at on Windows: the `PATH`.
#[cfg(not(target_os = "macos"))]
pub const fn search_basis() -> SearchBasis {
  SearchBasis::Path
}

/// What a search of the search path looks at on macOS: the login shell's `PATH` only once it was read, waiting for the
/// first read.
#[cfg(target_os = "macos")]
pub fn search_basis() -> SearchBasis {
  macos::basis()
}
