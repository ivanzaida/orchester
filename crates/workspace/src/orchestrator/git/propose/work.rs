//! A fingerprint of the work a proposal would discard, read when the card is made and again when it is applied, so an
//! edit since is never discarded unseen. Only a proposal that discards work takes one (a hard reset, discarding
//! changes, a switch that discards them, a clean, aborting a merge): a branch's or tag's deletion and a forced push
//! stay valid over an unrelated edit. It is cheap: git's raw list of the changes (modes, ids, status) and its hash of
//! each changed or untracked file's contents, never a patch; and it is bounded: more than [`MAX_FILES`] files or
//! [`MAX_BYTES`] of them is refused (`too_large`).

use std::fs;

use super::super::command::{Git, refused};
use crate::{app_mcp::ToolError, messages::Message};

/// The most files a fingerprint hashes.
const MAX_FILES: usize = 10_000;
/// The most bytes of files a fingerprint hashes.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Orchester's own folder, never part of the work.
const PROJECT_DATA: &str = ".orchester/";

/// The work a proposal discards.
pub(super) enum Work<'a> {
  /// The uncommitted changes to tracked files under `paths` (all of them without).
  Tracked(&'a [String]),
  /// Every uncommitted change to tracked files, and the untracked files `overwritten`.
  TrackedAnd { overwritten: &'a [String] },
  /// The untracked files under `paths` (all of them without), ignored ones too with `ignored`.
  Untracked { paths: &'a [String], ignored: bool },
}

/// The fingerprint of `work` in `git`'s folder, `with_head` when the repository has a commit to compare with.
pub(super) fn fingerprint(git: &Git, work: &Work, with_head: bool) -> Result<String, ToolError> {
  let against: &[&str] = if with_head { &["HEAD"] } else { &["--cached"] };
  let (raw, mut files) = match work {
    Work::Tracked(paths) => tracked(git, against, paths)?,
    Work::TrackedAnd { overwritten } => {
      let (raw, mut files) = tracked(git, against, &[])?;
      files.extend(overwritten.iter().cloned());
      (raw, files)
    }
    Work::Untracked { paths, ignored } => (String::new(), untracked(git, paths, *ignored)?),
  };
  files.retain(|file| !file.starts_with(PROJECT_DATA));
  files.sort();
  files.dedup();
  let contents = contents(git, &files)?;
  let text = format!("{raw}\0{}\0{contents}", files.join("\0"));
  Ok(git.ok_with(&["hash-object", "--stdin"], Some(&text))?.trim().to_owned())
}

/// git's raw list of the changes to tracked files under `paths` against `against`, and the changed files' names.
fn tracked(git: &Git, against: &[&str], paths: &[String]) -> Result<(String, Vec<String>), ToolError> {
  let scope = |first: &[&'static str]| {
    let mut args: Vec<&str> = first.to_vec();
    args.extend(against);
    args.push("--");
    args.extend(paths.iter().map(String::as_str));
    args
  };
  let raw = git.ok(&scope(&["diff", "--raw", "-z", "--no-ext-diff"]))?;
  let files = listed(&git.ok(&scope(&["diff", "--name-only", "-z", "--no-ext-diff"]))?);
  Ok((raw, files))
}

/// The untracked files under `paths`, ignored ones too with `ignored`.
fn untracked(git: &Git, paths: &[String], ignored: bool) -> Result<Vec<String>, ToolError> {
  let mut args = vec!["ls-files", "--others", "-z"];
  if !ignored {
    args.push("--exclude-standard");
  }
  args.push("--");
  args.extend(paths.iter().map(String::as_str));
  Ok(listed(&git.ok(&args)?))
}

/// git's hashes of the contents of `files` that are files in `git`'s folder, one per line; refused (`too_large`) over
/// the bounds.
fn contents(git: &Git, files: &[String]) -> Result<String, ToolError> {
  if files.len() > MAX_FILES {
    return Err(too_large());
  }
  let mut hashed = Vec::new();
  let mut bytes = 0_u64;
  for file in files {
    // A name git reads one per line cannot hold a line break: such a file counts by its name alone.
    if file.contains(['\n', '\r']) {
      continue;
    }
    if let Ok(metadata) = fs::metadata(git.dir.join(file))
      && metadata.is_file()
    {
      bytes += metadata.len();
      hashed.push(file.as_str());
    }
  }
  if bytes > MAX_BYTES {
    return Err(too_large());
  }
  if hashed.is_empty() {
    return Ok(String::new());
  }
  git.ok_with(
    &["hash-object", "--no-filters", "--stdin-paths"],
    Some(&hashed.join("\n")),
  )
}

/// The refusal of work over the bounds.
fn too_large() -> ToolError {
  refused(
    "too_large",
    Message::new("orchestrator.git.tooLarge")
      .with("files", MAX_FILES.to_string())
      .with("megabytes", (MAX_BYTES / (1024 * 1024)).to_string()),
  )
}

/// The names a `-z` list holds.
fn listed(text: &str) -> Vec<String> {
  text
    .split('\0')
    .filter(|name| !name.is_empty())
    .map(str::to_owned)
    .collect()
}
