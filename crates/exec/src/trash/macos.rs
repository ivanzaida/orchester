//! The Trash through Finder. A volume keeps what it is given in a Trash only on the person's home volume or one with a
//! `.Trashes` folder at its root; Finder deletes for good elsewhere, so other volumes are refused before anything
//! moves. Finder runs with a time limit, so a question it might ask never holds the caller.

use std::{
  fs, io,
  os::unix::fs::MetadataExt,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{command::Command, orchester_tools::OutputKept};

use super::Bin;

/// Finder moves the file or folder to the Trash; the path is the script's argument, never part of its text.
const SCRIPT: [&str; 3] = [
  "on run argv",
  "tell application \"Finder\" to delete (POSIX file (item 1 of argv) as alias)",
  "end run",
];
/// How long Finder may take before the move counts as failed and it is stopped.
const LIMIT: Duration = Duration::from_secs(60);
/// How much of Finder's answer is kept for its reason.
const REASON: usize = 4 * 1024;

pub fn ensure_movable(_path: &Path) -> io::Result<()> {
  // A file another program holds moves on macOS; nothing to refuse.
  Ok(())
}

pub fn bin_of(path: &Path) -> io::Result<Bin> {
  let device = fs::symlink_metadata(path)?.dev();
  let home = std::env::var_os("HOME").map(PathBuf::from);
  if home.is_some_and(|home| fs::metadata(home).is_ok_and(|home| home.dev() == device)) {
    return Ok(Bin::Keeps { capacity: None });
  }
  let mut root = path.to_path_buf();
  while let Some(parent) = root.parent() {
    if fs::metadata(parent)?.dev() != device {
      break;
    }
    root = parent.to_path_buf();
  }
  Ok(if root.join(".Trashes").is_dir() {
    Bin::Keeps { capacity: None }
  } else {
    Bin::Missing
  })
}

pub fn recycle(path: &Path) -> io::Result<()> {
  let mut args: Vec<&str> = SCRIPT.iter().flat_map(|line| ["-e", *line]).collect();
  let target = path
    .to_str()
    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
  args.push(target);
  let command = Command::new(Path::new("osascript"), args);
  let output = crate::run_capped(&command, LIMIT, (REASON, OutputKept::Tail)).map_err(io::Error::other)?;
  if output.code != Some(0) {
    let reason = output.stderr.trim();
    return Err(io::Error::other(if reason.is_empty() {
      "it was not moved to the Trash".to_owned()
    } else {
      reason.to_owned()
    }));
  }
  if fs::symlink_metadata(path).is_ok() {
    return Err(io::Error::other("it is still in its place after the move"));
  }
  Ok(())
}
