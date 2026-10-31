//! Handing over to the installer (`tools/installer/orchester.iss`): the app notes what it installs in
//! [`RECORD_FILE`] of the data folder, starts the verified installer silently with its pid to wait for, and exits. The
//! installer waits for that pid to exit, installs into the running copy's folder, and starts Orchester again; one that
//! fails writes why to [`RESULT_FILE`] and starts the old version again. The next start reads both
//! ([`super::outcome`]).

use std::{
  fs, io,
  path::{Path, PathBuf},
  process::{Child, Command, Stdio},
};

use orchester_api::releases::ReleaseVersion;
use serde::{Deserialize, Serialize};

/// The data folder's note of the update handed over to the installer, read by the next start.
pub const RECORD_FILE: &str = "installing-update.json";
/// Where a failed installer writes why, in the data folder.
pub const RESULT_FILE: &str = "installing-update-result.txt";
/// The folder of the data folder the installer's log goes to, with the app's own logs.
const LOGS_FOLDER: &str = "logs";

/// The update handed over to the installer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
  /// The version that handed over.
  pub from: ReleaseVersion,
  /// The version it installs.
  pub to: ReleaseVersion,
  /// Its release notes in Markdown, for What's new after the restart.
  pub notes: String,
  /// The installer's log.
  pub log: PathBuf,
}

impl InstallRecord {
  /// The record of installing `to` over `from`, with its log in `data_folder`'s logs.
  pub fn new(data_folder: &Path, from: ReleaseVersion, to: ReleaseVersion, notes: String) -> Self {
    let log = data_folder.join(LOGS_FOLDER).join(format!("update-{to}.log"));
    Self { from, to, notes, log }
  }

  /// Keeps the record in `data_folder`, and removes what an earlier installer left. Blocking.
  pub fn keep(&self, data_folder: &Path) -> io::Result<()> {
    if let Some(logs) = self.log.parent() {
      fs::create_dir_all(logs)?;
    }
    remove_if_present(&data_folder.join(RESULT_FILE))?;
    let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
    fs::write(data_folder.join(RECORD_FILE), text)
  }

  /// The record a handover kept in `data_folder`; `None` without one, or with one that cannot be read, which is
  /// logged and removed. Blocking.
  pub fn take(data_folder: &Path) -> Option<Self> {
    let path = data_folder.join(RECORD_FILE);
    let text = match fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
      Err(error) => {
        tracing::warn!(%error, "cannot read the note of the update handed over to the installer");
        return None;
      }
    };
    if let Err(error) = fs::remove_file(&path) {
      tracing::warn!(%error, "cannot remove the note of the update handed over to the installer");
    }
    serde_json::from_str(&text)
      .map_err(|error| tracing::warn!(%error, "the note of the update handed over to the installer is damaged"))
      .ok()
  }
}

/// How the app starts the installer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handover {
  /// The verified installer.
  pub installer: PathBuf,
  /// The folder the running copy was installed into, which the update replaces.
  pub program_folder: PathBuf,
  /// The app's process, which the installer waits for.
  pub pid: u32,
  /// The installer's log.
  pub log: PathBuf,
  /// Where a failed installer writes why.
  pub result: PathBuf,
}

impl Handover {
  /// The installer's arguments, as its command line holds them: silent, without message boxes or a restart of
  /// Windows, into the running copy's folder, logging, waiting for the app's pid to exit and starting Orchester again
  /// when done.
  pub fn arguments(&self) -> Vec<String> {
    vec![
      "/VERYSILENT".to_owned(),
      "/SUPPRESSMSGBOXES".to_owned(),
      "/NORESTART".to_owned(),
      quoted("/DIR=", &self.program_folder),
      quoted("/LOG=", &self.log),
      format!("/WAITPID={}", self.pid),
      "/RELAUNCH".to_owned(),
      quoted("/UPDATERESULT=", &self.result),
    ]
  }

  /// Starts the installer; it waits for the app to exit before it changes anything. On Windows it is started outside
  /// the app's job when the job allows that, so the app's exit does not end it.
  pub fn start(&self) -> io::Result<Child> {
    started(self, true).or_else(|error| {
      tracing::info!(%error, "cannot start the installer outside the app's job; starting it inside");
      started(self, false)
    })
  }
}

fn started(handover: &Handover, breakaway: bool) -> io::Result<Child> {
  let mut command = Command::new(&handover.installer);
  command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
  pass(&mut command, &handover.arguments(), breakaway);
  command.spawn()
}

/// `name` followed by `path` in quotes, as Inno Setup reads a path that may hold spaces.
fn quoted(name: &str, path: &Path) -> String {
  format!("{name}\"{}\"", path.display())
}

/// Inno Setup reads its command line itself: the arguments go as they are written, without Rust's quoting, and the
/// installer gets no console and its own process group.
#[cfg(windows)]
fn pass(command: &mut Command, arguments: &[String], breakaway: bool) {
  use std::os::windows::process::CommandExt;
  const DETACHED_PROCESS: u32 = 0x0000_0008;
  const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
  const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
  for argument in arguments {
    command.raw_arg(argument);
  }
  let breakaway = if breakaway { CREATE_BREAKAWAY_FROM_JOB } else { 0 };
  command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | breakaway);
}

#[cfg(not(windows))]
fn pass(command: &mut Command, arguments: &[String], _: bool) {
  command.args(arguments);
}

fn remove_if_present(path: &Path) -> io::Result<()> {
  match fs::remove_file(path) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}
