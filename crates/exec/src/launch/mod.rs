//! What actually starts when a program is run. On Windows, npm installs CLIs such as `codex` as
//! `.cmd` scripts; `cmd.exe` runs those by reading their arguments again, which splits or changes
//! free-form text. A script whose structure is recognised as npm's Node.js launcher is replaced by
//! the Node.js program it starts; any other script still runs through `cmd.exe`, but only with
//! arguments that pass through it unchanged. macOS and other Unix systems run every program
//! directly.

#[cfg(windows)]
mod command_script;

use std::path::{Path, PathBuf};

use orchester_core::command::CommandError;

#[cfg(windows)]
pub use self::command_script::{node_shim_script, passes_through_cmd};

/// How a program is started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Via {
  /// The program file itself.
  Direct,
  /// Node.js running `script`, the target of an npm launcher script.
  NodeScript { script: PathBuf },
  /// `cmd.exe` running a `.cmd`/`.bat` script that is not a recognised launcher.
  CommandScript,
}

/// The program that starts, with the arguments that come before the caller's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
  program: PathBuf,
  leading_args: Vec<String>,
  via: Via,
}

impl Launch {
  fn direct(program: &Path) -> Self {
    Self {
      program: program.to_path_buf(),
      leading_args: Vec::new(),
      via: Via::Direct,
    }
  }

  /// The file the operating system starts: the program itself, Node.js or the script.
  pub fn program(&self) -> &Path {
    &self.program
  }

  pub fn leading_args(&self) -> &[String] {
    &self.leading_args
  }

  pub fn via(&self) -> &Via {
    &self.via
  }

  /// Refuses `args` that `cmd.exe` would change on the way to a script's program.
  pub fn check_args(&self, args: &[String]) -> Result<(), CommandError> {
    if self.via != Via::CommandScript {
      return Ok(());
    }
    check_script_args(args)
  }
}

#[cfg(windows)]
fn check_script_args(args: &[String]) -> Result<(), CommandError> {
  match args.iter().position(|arg| !passes_through_cmd(arg)) {
    Some(position) => Err(CommandError::UnsafeScriptArgument { position }),
    None => Ok(()),
  }
}

#[cfg(not(windows))]
fn check_script_args(_args: &[String]) -> Result<(), CommandError> {
  Ok(())
}

/// What starts when `program` is run. Reads the file of a Windows command script; everything
/// else, and every program on macOS, starts directly.
pub fn resolve(program: &Path) -> Launch {
  #[cfg(windows)]
  if let Some(launch) = command_script::resolve(program) {
    return launch;
  }
  Launch::direct(program)
}
