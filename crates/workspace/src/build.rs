//! What kind of program this is, as it was built: the installed app — a build `tools/install.ps1` or
//! `tools/package-macos.sh` made, which set [`INSTALLED_BUILD_VARIABLE`] while compiling — or any other build (debug,
//! `cargo run --release`, an example, a test). Only other builds take the variables that point a QA copy at folders
//! of its own (the app's MCP discovery folder, `ORCHESTER_ACCOUNTS_DIR`); the installed app keeps to the person's.

/// Set while compiling the installed app, by `tools/install.ps1` and `tools/package-macos.sh` only.
pub const INSTALLED_BUILD_VARIABLE: &str = "ORCHESTER_APP_INSTALLED_BUILD";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
  /// The installed app.
  Installed,
  /// Any other build.
  Other,
}

impl Build {
  /// This program's kind: a release build compiled with [`INSTALLED_BUILD_VARIABLE`] set to `1`.
  pub fn this() -> Self {
    Self::of(option_env!("ORCHESTER_APP_INSTALLED_BUILD"), cfg!(debug_assertions))
  }

  /// The kind of a build compiled with [`INSTALLED_BUILD_VARIABLE`] as `flag`, as a `debug` build or not. Only the
  /// exact `1` in a release build counts: a variable left in a shell (`0`, empty) or a debug build that inherited it
  /// never becomes the installed app, takes its port or stores a token.
  pub fn of(flag: Option<&str>, debug: bool) -> Self {
    match (flag, debug) {
      (Some("1"), false) => Self::Installed,
      _ => Self::Other,
    }
  }
}
