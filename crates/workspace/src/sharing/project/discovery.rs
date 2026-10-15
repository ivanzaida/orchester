//! `orchester.toml`, the optional discovery file at the repository root (`docs/architecture/
//! server.md`, decisions 11 and 49, "Security"): `project = "https://…/kontur"` names the project's
//! address, which prefills linking a colleague's checkout; files written before API 3 name only the
//! server (`server = "https://…"`). It comes from the repository and is untrusted: reading it never
//! contacts the address; the person confirms it first. Orchester writes it when the person asks
//! (Share) and never commits it.

use std::{fs, io::Write, path::Path, time::SystemTime};

use orchester_api::client::ServerAddress;
use orchester_core::{command::Command, time::Timestamp};
use orchester_exec::programs::Programs;
use serde::Deserialize;

use crate::sharing::ProjectAddress;

/// The file's name at the repository root.
pub const FILE: &str = "orchester.toml";

#[derive(Deserialize)]
struct Contents {
  project: Option<String>,
  server: Option<String>,
}

/// What the repository's discovery file says, as the frozen `crates/desktop` reads it: the server only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Discovery {
  /// There is no file, or it names no server.
  Absent,
  /// It names this server, or a project on it.
  Server(ServerAddress),
  /// It cannot be read, or what it names is not an address.
  Unreadable,
}

/// The discovery file of a shared project, as Sharing shows it (10.13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryFile {
  pub discovery: Discovery,
  /// When the file was last written.
  pub written_at: Option<Timestamp>,
  /// Whether git has it as committed; `None` when git could not say.
  pub committed: Option<bool>,
}

/// Why the discovery file was not written.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DiscoveryError {
  /// The repository has the file already and it names another project or server; it is left as it is.
  #[error("orchester.toml names another project")]
  Other,
  /// The repository's `orchester.toml` is a link; Orchester writes no file through it.
  #[error("orchester.toml is a link")]
  Link,
  #[error("orchester.toml cannot be written: {0}")]
  Write(String),
}

/// What the repository's discovery file says since API 3 (row 49).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectDiscovery {
  /// There is no file, or it names nothing.
  Absent,
  /// It names the project at this address, which prefills linking the checkout.
  Project(ProjectAddress),
  /// It names only a server (a file written before API 3): the person picks the project there.
  ServerOnly(ServerAddress),
  /// It cannot be read, or what it names is not an address.
  Unreadable,
}

/// Reads the discovery file of the repository at `repo_root`. Blocking; nothing is contacted.
pub fn project(repo_root: &Path) -> ProjectDiscovery {
  let text = match fs::read_to_string(repo_root.join(FILE)) {
    Ok(text) => text,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ProjectDiscovery::Absent,
    Err(_) => return ProjectDiscovery::Unreadable,
  };
  match toml::from_str::<Contents>(&text) {
    Ok(Contents {
      project: Some(project), ..
    }) => ProjectAddress::parse(&project).map_or(ProjectDiscovery::Unreadable, ProjectDiscovery::Project),
    Ok(Contents {
      server: Some(server), ..
    }) => ServerAddress::parse(&server).map_or(ProjectDiscovery::Unreadable, ProjectDiscovery::ServerOnly),
    Ok(_) => ProjectDiscovery::Absent,
    Err(_) => ProjectDiscovery::Unreadable,
  }
}

/// [`project`] as the frozen `crates/desktop` reads it: the server only. Blocking; nothing is contacted.
pub fn read(repo_root: &Path) -> Discovery {
  match project(repo_root) {
    ProjectDiscovery::Absent => Discovery::Absent,
    ProjectDiscovery::Project(address) => Discovery::Server(address.server().clone()),
    ProjectDiscovery::ServerOnly(server) => Discovery::Server(server),
    ProjectDiscovery::Unreadable => Discovery::Unreadable,
  }
}

/// The discovery file with when it was written and whether it is committed. Blocking.
pub fn inspect(programs: &Programs, repo_root: &Path) -> DiscoveryFile {
  // Without a modification time the file is shown without when it was written.
  let written_at = fs::metadata(repo_root.join(FILE))
    .and_then(|metadata| metadata.modified())
    .ok()
    .and_then(|modified| modified.duration_since(SystemTime::UNIX_EPOCH).ok())
    .and_then(|since| i64::try_from(since.as_millis()).ok())
    .map(Timestamp::from_unix_millis);
  let status = Command::new(Path::new("git"), ["status", "--porcelain", "--", FILE])
    .with_current_dir(repo_root)
    .with_env("LC_ALL", "C");
  // git that cannot run or answer leaves the commit state unknown, which Sharing says.
  let committed = programs
    .runner()
    .run(&status)
    .ok()
    .filter(|output| output.success())
    .map(|output| output.stdout.trim().is_empty());
  DiscoveryFile {
    discovery: read(repo_root),
    written_at,
    committed,
  }
}

/// Writes `project = "<address>"` into the repository's discovery file. A file that names this
/// project already is left as it is; one that names anything else is never overwritten; a link, even
/// one that leads nowhere, is never followed. The file is created only if nothing is at its path.
pub fn write(repo_root: &Path, address: &str) -> Result<(), DiscoveryError> {
  let path = repo_root.join(FILE);
  if fs::symlink_metadata(&path).is_ok_and(|found| found.file_type().is_symlink()) {
    return Err(DiscoveryError::Link);
  }
  match project(repo_root) {
    ProjectDiscovery::Project(found) if found.to_text() == address => return Ok(()),
    ProjectDiscovery::Project(_) | ProjectDiscovery::ServerOnly(_) | ProjectDiscovery::Unreadable => {
      return Err(DiscoveryError::Other);
    }
    ProjectDiscovery::Absent if repo_root.join(FILE).exists() => return Err(DiscoveryError::Other),
    ProjectDiscovery::Absent => {}
  }
  let quoted = address.replace('\\', "\\\\").replace('"', "\\\"");
  let written = fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&path)
    .and_then(|mut file| file.write_all(format!("project = \"{quoted}\"\n").as_bytes()));
  written.map_err(|error| match error.kind() {
    std::io::ErrorKind::AlreadyExists => DiscoveryError::Other,
    _ => DiscoveryError::Write(error.to_string()),
  })
}
