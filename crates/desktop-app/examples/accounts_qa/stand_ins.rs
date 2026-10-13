//! The stand-in `claude` and `codex` the QA window checks its accounts with, so no real CLI is asked and no real
//! sign-in is read. They answer `--version` and their status command: signed in when the folder their variable names
//! holds a `signed-in` file, the stand-in `claude` as the email and plan written in it; the stand-in `claude` answers
//! in a way Orchester can't read when the folder holds `garbled`.

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

/// Marks `folder` signed in for the stand-ins, as `email` on `plan` for the stand-in `claude`.
pub fn sign_in(folder: &Path, (email, plan): (&str, &str)) -> io::Result<()> {
  fs::write(folder.join("signed-in"), format!("{email} {plan}\n"))
}

/// Makes the stand-in `claude` answer `folder`'s status in a way Orchester can't read.
pub fn garble(folder: &Path) -> io::Result<()> {
  fs::write(folder.join("garbled"), "")
}

#[cfg(windows)]
const CLAUDE_SCRIPT: &str = r#"@echo off
if "%1"=="--version" goto version
if "%1"=="auth" if "%2"=="status" goto status
exit /b 1
:version
echo 2.1.296 ^(Claude Code^)
exit /b 0
:status
if exist "%CLAUDE_CONFIG_DIR%\garbled" goto garbled
if exist "%CLAUDE_CONFIG_DIR%\signed-in" goto in
echo {"loggedIn":false,"authMethod":"none"}
exit /b 1
:garbled
echo Welcome back.
exit /b 0
:in
set /p WHO=<"%CLAUDE_CONFIG_DIR%\signed-in"
for /f "tokens=1,2" %%a in ("%WHO%") do echo {"loggedIn":true,"authMethod":"claude.ai","email":"%%a","subscriptionType":"%%b"}
exit /b 0
"#;

#[cfg(windows)]
const CODEX_SCRIPT: &str = r#"@echo off
if "%1"=="--version" goto version
if "%1"=="login" goto login
exit /b 1
:version
echo codex-cli 0.159.2
exit /b 0
:login
if exist "%CODEX_HOME%\signed-in" goto in
echo Not logged in
exit /b 1
:in
echo Logged in using ChatGPT
exit /b 0
"#;

#[cfg(not(windows))]
const CLAUDE_SCRIPT: &str = r#"#!/bin/sh
case "$1 $2" in
  --version*) echo '2.1.296 (Claude Code)' ;;
  'auth status')
    if [ -e "$CLAUDE_CONFIG_DIR/garbled" ]; then echo 'Welcome back.'
    elif [ -e "$CLAUDE_CONFIG_DIR/signed-in" ]; then
      read -r email plan < "$CLAUDE_CONFIG_DIR/signed-in"
      printf '{"loggedIn":true,"authMethod":"claude.ai","email":"%s","subscriptionType":"%s"}\n' "$email" "$plan"
    else echo '{"loggedIn":false,"authMethod":"none"}'; exit 1; fi ;;
  *) exit 1 ;;
esac
"#;

#[cfg(not(windows))]
const CODEX_SCRIPT: &str = r#"#!/bin/sh
case "$1" in
  --version) echo 'codex-cli 0.159.2' ;;
  login) if [ -e "$CODEX_HOME/signed-in" ]; then echo 'Logged in using ChatGPT';
    else echo 'Not logged in'; exit 1; fi ;;
  *) exit 1 ;;
esac
"#;

#[cfg(windows)]
fn write_program(path: &Path, script: &str) -> io::Result<()> {
  // cmd reads a batch file's lines ended with CRLF.
  fs::write(path, script.replace('\n', "\r\n"))
}

#[cfg(not(windows))]
fn write_program(path: &Path, script: &str) -> io::Result<()> {
  use std::os::unix::fs::PermissionsExt;
  fs::write(path, script)?;
  fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
