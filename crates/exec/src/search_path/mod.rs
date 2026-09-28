//! Where programs are looked up (`PATH`).
//!
//! An app started from Finder, the Dock or Spotlight on macOS inherits launchd's minimal `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`), not the one the user's shell builds. The agent CLIs, `gh`,
//! Node.js (which npm-installed CLIs start through `#!/usr/bin/env node`) and the tools agents run
//! are usually elsewhere: Homebrew, npm, nvm, `~/.local/bin`. So on macOS the search path is the
//! inherited `PATH` followed by the usual install folders that exist ([`macos`]). Connection
//! detection searches it, and every program Orchester starts gets it as its `PATH`, the runner
//! included, with the program's own folder last. Folders already on the `PATH` keep their place,
//! so a `PATH` from a terminal (`cargo run`) wins. The user's shell is not asked for its `PATH`:
//! that would run their shell start-up files on every app start.
//!
//! Windows programs inherit the user's full `PATH`, which is used as it is.

#[cfg(target_os = "macos")]
pub mod macos;

use std::ffi::OsString;

/// The search path for programs: this process's `PATH`.
#[cfg(not(target_os = "macos"))]
pub fn search_path() -> Option<OsString> {
  std::env::var_os("PATH")
}

/// The search path for programs: this process's `PATH` followed by the usual install folders.
#[cfg(target_os = "macos")]
pub fn search_path() -> Option<OsString> {
  let folders = macos::usual_folders(macos::home().as_deref());
  Some(macos::extend(std::env::var_os("PATH").as_deref(), &folders))
}
