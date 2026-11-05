//! Where the user's discovery folder is, and how a file only the user can read is made, per
//! platform. On Windows the folder is in the local app data folder, whose access the user, SYSTEM
//! and administrators have and files in it inherit; the file admits the user and the account the app
//! runs as. On macOS it is in the user's Library, the folder is made `0700` and the file `0600` as
//! they are created, before the token is written: the account the app runs as owns both, and a
//! sandbox there (Seatbelt) runs as the same account.

use std::{
  fs::{self, File},
  io,
  path::{Path, PathBuf},
};

/// The discovery folder, named apart from lurq's `lurq/mcp`, from the deprecated desktop's `orchester-mcp` (which the
/// installed stable build writes) and from the app's data folder.
const FOLDER: &str = "orchester-app-mcp";

/// `%LOCALAPPDATA%\orchester-app-mcp`.
#[cfg(windows)]
pub(super) fn user_folder() -> Option<PathBuf> {
  let folder = std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join(FOLDER));
  if folder.is_none() {
    tracing::error!("LOCALAPPDATA is not set; the app's MCP server writes no discovery file");
  }
  folder
}

/// `~/Library/Application Support/orchester-app-mcp`.
#[cfg(target_os = "macos")]
pub(super) fn user_folder() -> Option<PathBuf> {
  let folder = std::env::var_os("HOME").map(|home| {
    PathBuf::from(home)
      .join("Library")
      .join("Application Support")
      .join(FOLDER)
  });
  if folder.is_none() {
    tracing::error!("HOME is not set; the app's MCP server writes no discovery file");
  }
  folder
}

/// Orchester ships on Windows and macOS only; elsewhere the server has no discovery folder and says so.
#[cfg(not(any(windows, target_os = "macos")))]
pub(super) fn user_folder() -> Option<PathBuf> {
  tracing::error!("the app's MCP server has no discovery folder on this platform yet");
  None
}

#[cfg(windows)]
pub(super) fn create_private_folder(folder: &Path) -> io::Result<()> {
  fs::create_dir_all(folder)
}

#[cfg(unix)]
pub(super) fn create_private_folder(folder: &Path) -> io::Result<()> {
  use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

  fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)?;
  // A folder made by an earlier version, or by hand, may admit others.
  fs::set_permissions(folder, fs::Permissions::from_mode(0o700))
}

/// Created only where no file or link is, then given an access list of its own that admits only the
/// user and the account the app runs as (inheritance removed), before anything is written to it.
#[cfg(windows)]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
  let file = File::options().write(true).create_new(true).open(path)?;
  if let Err(error) = admit_only_the_user(path) {
    drop(file);
    // The file holds nothing yet; without its access list it must not hold the token either.
    if let Err(removal) = fs::remove_file(path) {
      tracing::warn!(%removal, file = %path.display(), "cannot remove a discovery file without its access list");
    }
    return Err(error);
  }
  Ok(file)
}

/// How long icacls and whoami may take.
#[cfg(windows)]
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// `icacls <path> /inheritance:r /grant:r <user>:F *<account>:F`: the user and the account this process runs as, and
/// nobody else, may use `path`. The user is named as `%USERDOMAIN%\%USERNAME%`; the account is the process token's,
/// by SID ([`own_account`]). They differ when the app runs as another account than the person's environment names: a
/// QA copy started inside Codex's Windows sandbox runs as its sandbox user, which reads the file the copy writes, and
/// the person's runner relays to the copy through the file as well. icacls ships with Windows and runs as
/// `%SystemRoot%\System32\icacls.exe`, through `orchester-exec`, which hides its console.
#[cfg(windows)]
fn admit_only_the_user(path: &Path) -> io::Result<()> {
  use orchester_core::command::{Command, CommandRunner};

  let user = std::env::var("USERNAME").map_err(|_| io::Error::other("USERNAME is not set"))?;
  let user = match std::env::var("USERDOMAIN") {
    Ok(domain) if !domain.is_empty() => format!(r"{domain}\{user}"),
    _ => user,
  };
  let account = own_account()?;
  let file = path
    .to_str()
    .ok_or_else(|| io::Error::other("the discovery file's path is not Unicode"))?;
  let command = Command::new(
    &system_program("icacls.exe")?,
    [
      file,
      "/inheritance:r",
      "/grant:r",
      &format!("{user}:F"),
      &format!("*{account}:F"),
    ],
  );
  let output = orchester_exec::ProcessRunner::new(TIMEOUT)
    .run(&command)
    .map_err(io::Error::other)?;
  if output.success() {
    return Ok(());
  }
  Err(io::Error::other(format!(
    "icacls could not limit the discovery file to {user} and {account}: {}",
    output.stderr.trim()
  )))
}

/// The SID of the account this process's token runs as, from `whoami /user /fo csv /nh`, which answers
/// `"<domain>\<name>","<SID>"` (`%SystemRoot%\System32\whoami.exe`, through `orchester-exec`).
#[cfg(windows)]
fn own_account() -> io::Result<String> {
  use orchester_core::command::{Command, CommandRunner};

  let command = Command::new(&system_program("whoami.exe")?, ["/user", "/fo", "csv", "/nh"]);
  let output = orchester_exec::ProcessRunner::new(TIMEOUT)
    .run(&command)
    .map_err(io::Error::other)?;
  if !output.success() {
    return Err(io::Error::other(format!(
      "whoami could not name this process's account: {}",
      output.stderr.trim()
    )));
  }
  output
    .stdout
    .trim()
    .rsplit(',')
    .next()
    .map(|field| field.trim().trim_matches('"'))
    .filter(|sid| {
      sid
        .strip_prefix("S-1-")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-'))
    })
    .map(str::to_owned)
    .ok_or_else(|| io::Error::other("whoami named this process's account without a SID"))
}

/// `%SystemRoot%\System32\<program>`.
#[cfg(windows)]
fn system_program(program: &str) -> io::Result<PathBuf> {
  let system_root = std::env::var_os("SystemRoot").ok_or_else(|| io::Error::other("SystemRoot is not set"))?;
  Ok(PathBuf::from(system_root).join("System32").join(program))
}

#[cfg(unix)]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
  use std::os::unix::fs::OpenOptionsExt;

  File::options().write(true).create_new(true).mode(0o600).open(path)
}
