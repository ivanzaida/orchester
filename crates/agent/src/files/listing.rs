//! `list_files` (`docs/architecture/native-agent.md` §4.2): the files under a folder that match a glob,
//! sorted, at most 1000. ripgrep lists them (`rg --files`), which respects `.gitignore`; without it the
//! folder is walked. A caller that knows the files already (the code index's list and the worktree's
//! changes since) has them filtered the same way instead ([`FileTools::list_known_files`]).

use std::{
  path::{Path, PathBuf},
  time::Instant,
};

use orchester_core::{
  connection::stream::CallDetail,
  native::{session::ToolOutcome, tools::ListFiles},
  orchester_tools::plain_relative,
};

use super::{
  FileError, FileTools, count, done,
  glob::Glob,
  outcome,
  ripgrep::{self, Want},
  walk::{self, MAX_WALKED, WALK_TIMEOUT},
};

/// Entries one result lists.
const MAX_ENTRIES: usize = 1000;
/// What a result without ripgrep says.
pub(super) const NO_RIPGREP: &str = "[ripgrep was not found: folders were walked without .gitignore rules]";

/// Files found, and whether more were left out.
struct Found {
  paths: Vec<PathBuf>,
  more: bool,
}

impl FileTools {
  pub fn list_files(&self, input: &ListFiles) -> ToolOutcome {
    outcome(self.list(input, None))
  }

  /// `list_files` over `known`, the files the caller knows the worktree holds (relative to it, with `/`), for
  /// a folder of the worktree; any other folder is listed as always. Each known path must be a plain relative
  /// path that resolves in the scope as `read_file` resolves it; the others are left out, and counted.
  pub fn list_known_files(&self, input: &ListFiles, known: &[String]) -> ToolOutcome {
    let mut dropped = 0;
    let known: Vec<PathBuf> = known
      .iter()
      .filter_map(|path| {
        let resolved = plain_relative(path).then(|| self.scope.readable(path).ok()).flatten();
        dropped += usize::from(resolved.is_none());
        resolved
      })
      .collect();
    outcome(self.list(input, Some((&known, dropped))))
  }

  fn list(&self, input: &ListFiles, known: Option<(&[PathBuf], usize)>) -> Result<ToolOutcome, FileError> {
    let folder = self.scope.readable(input.path().unwrap_or("."))?;
    if !folder.is_dir() {
      return Err(if folder.exists() {
        FileError::NotFolder(self.shown(&folder))
      } else {
        FileError::Missing(self.shown(&folder))
      });
    }
    // What the caller knows covers the worktree only: the code index, the skills and the extra writable
    // folders are listed as always.
    let known = known.filter(|_| folder.starts_with(self.scope.worktree()));
    let found = match (known, &self.ripgrep) {
      (Some((known, _)), _) => filtered(known, &folder, input.pattern())?,
      (None, Some(program)) => listed(program, &folder, input.pattern())?,
      (None, None) => walked(&folder, input.pattern())?,
    };
    let mut shown: Vec<String> = found.paths.iter().map(|path| self.shown(path)).collect();
    shown.sort();
    let total = shown.len();
    shown.truncate(MAX_ENTRIES);
    let mut text = if shown.is_empty() {
      "no files match".to_owned()
    } else {
      shown.join("\n")
    };
    if total > MAX_ENTRIES || found.more {
      let counted = if found.more {
        format!("more than {total}")
      } else {
        total.to_string()
      };
      text.push_str(&format!(
        "\n[truncated: {counted} files match; showing the first {MAX_ENTRIES}. Narrow the pattern or path]"
      ));
    }
    if let Some((_, dropped)) = known.filter(|(_, dropped)| *dropped > 0) {
      text.push_str(&format!(
        "\n[{dropped} entries of the code index or of git were no paths in the worktree and were left out]"
      ));
    }
    if known.is_none() && self.ripgrep.is_none() {
      text.push('\n');
      text.push_str(NO_RIPGREP);
    }
    let detail = CallDetail::Listed { entries: count(total) };
    Ok(done(text, format!("{total} files"), Vec::new(), detail))
  }
}

/// The files `rg --files` lists under `folder`.
fn listed(program: &Path, folder: &Path, pattern: Option<&str>) -> Result<Found, FileError> {
  let mut args = vec!["--files".to_owned()];
  if let Some(pattern) = pattern {
    args.extend(["--glob".to_owned(), pattern.to_owned()]);
  }
  args.extend(["--".to_owned(), folder.display().to_string()]);
  let mut paths = Vec::new();
  let finished = ripgrep::run(program, &args, folder, |line| {
    paths.push(PathBuf::from(line));
    if paths.len() >= MAX_WALKED {
      Want::Enough
    } else {
      Want::More
    }
  })?;
  if finished.code == Some(2) && paths.is_empty() {
    return Err(FileError::Ripgrep(finished.stderr));
  }
  Ok(Found {
    paths,
    more: finished.stopped,
  })
}

/// The files under `folder` that match `pattern`, walked.
fn walked(folder: &Path, pattern: Option<&str>) -> Result<Found, FileError> {
  let deadline = Instant::now() + WALK_TIMEOUT;
  let (files, mut more) = walk::files(folder, deadline);
  let glob = pattern.map(Glob::new).transpose()?;
  let mut paths = Vec::new();
  for path in files {
    if Instant::now() > deadline {
      more = true;
      break;
    }
    if glob.as_ref().is_none_or(|glob| glob.matches(&relative(folder, &path))) {
      paths.push(path);
    }
  }
  Ok(Found { paths, more })
}

/// The files of `known` under `folder` that exist and match `pattern`.
fn filtered(known: &[PathBuf], folder: &Path, pattern: Option<&str>) -> Result<Found, FileError> {
  let glob = pattern.map(Glob::new).transpose()?;
  let paths = known
    .iter()
    .filter(|path| path.starts_with(folder) && path.is_file())
    .filter(|path| glob.as_ref().is_none_or(|glob| glob.matches(&relative(folder, path))))
    .cloned()
    .collect();
  Ok(Found { paths, more: false })
}

/// `path` relative to `folder`, with `/` separators.
pub(super) fn relative(folder: &Path, path: &Path) -> String {
  let inside = path.strip_prefix(folder).unwrap_or(path);
  inside
    .components()
    .map(|component| component.as_os_str().to_string_lossy())
    .collect::<Vec<_>>()
    .join("/")
}
