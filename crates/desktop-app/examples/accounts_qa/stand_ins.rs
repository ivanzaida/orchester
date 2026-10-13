//! The stand-in `claude` and `codex` the QA window checks its accounts with, so no real CLI is asked and no real
//! sign-in is read. They answer `--version` and their status command: signed in when the folder their variable names
//! holds a `signed-in` file; the stand-in `claude` answers in a way Orchester can't read when it holds `garbled`.

use std::{fs, io, path::Path};

/// The stand-ins' names on this platform.
#[cfg(windows)]
pub const CLAUDE: &str = "claude.cmd";
#[cfg(windows)]
pub const CODEX: &str = "codex.cmd";
#[cfg(not(windows))]
pub const CLAUDE: &str = "claude";
#[cfg(not(windows))]
pub const CODEX: &str = "codex";

/// Writes the stand-in `claude` (and `codex`, unless `without_codex`) into `folder`.
pub fn write(folder: &Path, without_codex: bool) -> io::Result<()> {
  fs::create_dir_all(folder)?;
  write_program(&folder.join(CLAUDE), CLAUDE_SCRIPT)?;
  if !without_codex {
    write_program(&folder.join(CODEX), CODEX_SCRIPT)?;
  }
  Ok(())
}

/// Marks `folder` signed in for the stand-ins.
pub fn sign_in(folder: &Path) -> io::Result<()> {
  fs::write(folder.join("signed-in"), "")
}

/// Makes the stand-in `claude` answer `folder`'s status in a way Orchester can't read.
pub fn garble(folder: &Path) -> io::Result<()> {
  fs::write(folder.join("garbled"), "")
}

#[cfg(windows)]
const CLAUDE_SCRIPT: &str = "@echo off\r\n\
  if \"%1\"==\"--version\" goto version\r\n\
  if \"%1\"==\"auth\" if \"%2\"==\"status\" goto status\r\n\
  exit /b 1\r\n\
  :version\r\n\
  echo 2.1.296 ^(Claude Code^)\r\n\
  exit /b 0\r\n\
  :status\r\n\
  if exist \"%CLAUDE_CONFIG_DIR%\\garbled\" goto garbled\r\n\
  if exist \"%CLAUDE_CONFIG_DIR%\\signed-in\" goto in\r\n\
  echo {\"loggedIn\":false,\"authMethod\":\"none\"}\r\n\
  exit /b 1\r\n\
  :garbled\r\n\
  echo Welcome back.\r\n\
  exit /b 0\r\n\
  :in\r\n\
  echo {\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"email\":\"ana@example.com\",\"subscriptionType\":\"max\"}\r\n\
  exit /b 0\r\n";

#[cfg(windows)]
const CODEX_SCRIPT: &str = "@echo off\r\n\
  if \"%1\"==\"--version\" goto version\r\n\
  if \"%1\"==\"login\" goto login\r\n\
  exit /b 1\r\n\
  :version\r\n\
  echo codex-cli 0.159.2\r\n\
  exit /b 0\r\n\
  :login\r\n\
  if exist \"%CODEX_HOME%\\signed-in\" goto in\r\n\
  echo Not logged in\r\n\
  exit /b 1\r\n\
  :in\r\n\
  echo Logged in using ChatGPT\r\n\
  exit /b 0\r\n";

#[cfg(not(windows))]
const CLAUDE_SCRIPT: &str = "#!/bin/sh\n\
  case \"$1 $2\" in\n\
    --version*) echo '2.1.296 (Claude Code)' ;;\n\
    'auth status') if [ -e \"$CLAUDE_CONFIG_DIR/garbled\" ]; then echo 'Welcome back.'; \
      elif [ -e \"$CLAUDE_CONFIG_DIR/signed-in\" ]; then \
      echo '{\"loggedIn\":true,\"authMethod\":\"claude.ai\",\"email\":\"ana@example.com\",\"subscriptionType\":\"max\"}'; \
      else echo '{\"loggedIn\":false,\"authMethod\":\"none\"}'; exit 1; fi ;;\n\
    *) exit 1 ;;\n\
  esac\n";

#[cfg(not(windows))]
const CODEX_SCRIPT: &str = "#!/bin/sh\n\
  case \"$1\" in\n\
    --version) echo 'codex-cli 0.159.2' ;;\n\
    login) if [ -e \"$CODEX_HOME/signed-in\" ]; then echo 'Logged in using ChatGPT'; \
      else echo 'Not logged in'; exit 1; fi ;;\n\
    *) exit 1 ;;\n\
  esac\n";

#[cfg(windows)]
fn write_program(path: &Path, script: &str) -> io::Result<()> {
  fs::write(path, script)
}

#[cfg(not(windows))]
fn write_program(path: &Path, script: &str) -> io::Result<()> {
  use std::os::unix::fs::PermissionsExt;
  fs::write(path, script)?;
  fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
