//! What the project's environment (Project settings › Environment) tells Cargo, as the agent's CLI gets it: the host's
//! own Cargo variables are not passed on, so only the project's count. Cargo reads a variable by its exact name on
//! macOS and without regard to case on Windows, and so are they read here. Besides where a build writes
//! (`CARGO_TARGET_DIR`, else `CARGO_BUILD_TARGET_DIR`), an alias (`CARGO_ALIAS_<NAME>`) may stand in for a subcommand,
//! and a relative `CARGO_HOME` leaves the configuration Cargo reads unknown. The variables that change what a build
//! runs are named to auto mode's reviewer ([`build_variables`]), never their values.

use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use crate::resource::CommandEnvironment;

/// The variables Cargo, rustc and rustup read that change what a build runs, by their exact names.
const RUNS: [&str; 12] = [
  "RUSTFLAGS",
  "RUSTDOCFLAGS",
  "RUSTC_WRAPPER",
  "RUSTC_WORKSPACE_WRAPPER",
  "RUSTC",
  "RUSTDOC",
  "RUSTUP_TOOLCHAIN",
  "CARGO_ENCODED_RUSTFLAGS",
  "CARGO_ENCODED_RUSTDOCFLAGS",
  "CARGO_BUILD_RUSTFLAGS",
  "CARGO_BUILD_RUSTDOCFLAGS",
  "CARGO_BUILD_RUSTDOC",
];
/// …and by their start (`CARGO_BUILD_RUSTC`, `CARGO_BUILD_RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER`)…
const RUNS_STARTING: [&str; 1] = ["CARGO_BUILD_RUSTC"];
/// …and a target's settings, `CARGO_TARGET_<TRIPLE>_<SETTING>`.
const TARGET_SETTINGS: [&str; 3] = ["_RUNNER", "_LINKER", "_RUSTFLAGS"];

/// What the agent's environment tells Cargo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct CargoEnvironment {
  /// The target directory it names, as written.
  pub(in super::super) target: Option<PathBuf>,
  /// The names its aliases stand for, in lower case.
  pub(super) aliases: Vec<String>,
  /// A Cargo home it names cannot be placed: what configuration Cargo reads is not known.
  pub(super) config_unknown: bool,
}

impl CargoEnvironment {
  /// What `environment` tells Cargo.
  pub(in super::super) fn of(environment: &CommandEnvironment) -> Self {
    let target = variable(environment, "CARGO_TARGET_DIR")
      .or_else(|| variable(environment, "CARGO_BUILD_TARGET_DIR"))
      .map(PathBuf::from);
    let aliases = environment
      .iter()
      .filter_map(|(name, _)| {
        cargo_name(name.as_str())
          .strip_prefix("CARGO_ALIAS_")
          .map(str::to_lowercase)
      })
      .collect();
    let config_unknown = variable(environment, "CARGO_HOME").is_some_and(|home| !Path::new(home).is_absolute());
    Self {
      target,
      aliases,
      config_unknown,
    }
  }

  /// Whether an alias may stand in for `subcommand`: one of its name, and for `clippy`, which is no built-in, any.
  pub(super) fn may_alias(&self, subcommand: &str, built_in: bool) -> bool {
    (!built_in && !self.aliases.is_empty()) || self.aliases.iter().any(|alias| alias.replace('_', "-") == subcommand)
  }
}

/// The value of `name` in `environment`, as Cargo reads it.
pub(in super::super) fn variable<'a>(environment: &'a CommandEnvironment, name: &str) -> Option<&'a str> {
  environment
    .iter()
    .find(|(variable, _)| cargo_name(variable.as_str()) == name)
    .map(|(_, value)| value)
}

/// `name` as Cargo compares it with the names it reads: in upper case on Windows, as written elsewhere.
fn cargo_name(name: &str) -> Cow<'_, str> {
  if cfg!(windows) {
    Cow::Owned(name.to_ascii_uppercase())
  } else {
    Cow::Borrowed(name)
  }
}

/// The names, as written, of the variables of `environment` that change what a build runs: its flags, the programs
/// it runs in place of rustc, rustdoc or the built program, the toolchain.
pub fn build_variables(environment: &CommandEnvironment) -> Vec<String> {
  environment
    .iter()
    .map(|(name, _)| name.as_str())
    .filter(|name| {
      let name = cargo_name(name);
      RUNS.contains(&name.as_ref())
        || RUNS_STARTING.iter().any(|start| name.starts_with(start))
        || (name.starts_with("CARGO_TARGET_") && TARGET_SETTINGS.iter().any(|setting| name.ends_with(setting)))
    })
    .map(str::to_owned)
    .collect()
}
