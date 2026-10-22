//! Whether a typed or picked path may become a source (`Sources · states` `lOOY6`, *Add source* `t4r78`): it must
//! name a local folder or file that exists and can be read, lie outside the project's own folder (its worktrees hold
//! it already) and outside every source it would join, and be nothing Orchester's reads refuse anywhere: a `.git`
//! folder, Orchester's own data or another project's `.orchester` state, or a credential location
//! (`orchester_core::orchester_tools::OutsideReads`, the rules `read_external_file` follows).

use std::{
  fs, io,
  path::{Component, Path, PathBuf},
};

use orchester_core::{
  native::scope::{ScopeError, in_git_folder},
  orchester_tools::{OutsideReadError, OutsideReads},
  permission::category::CommandScope,
  source::{ResolvedSources, RunSources, SourceError, SourceOrigin, SourcePath},
};

use crate::messages::Message;

/// The most entries of a folder the check walks to count its files.
pub const COUNT_LIMIT: usize = 10_000;
/// Orchester's state in a repository, refused in another project's folder.
const STATE_DIR: &str = ".orchester";

/// What a source names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  File,
  Folder,
}

/// A path that may become a source, as the check found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
  pub path: SourcePath,
  pub kind: Kind,
  /// The files of a folder, when there are at most [`COUNT_LIMIT`] entries to walk.
  pub files: Option<usize>,
  /// The folder holds a `.git` folder or Orchester state, which stay refused in it.
  pub holds_refused: bool,
}

impl Candidate {
  /// What the check found, said under the path (`Folder · 214 files · readable.`).
  pub fn message(&self) -> Message {
    match (self.kind, self.files) {
      (Kind::File, _) => Message::new("sources.check.file"),
      (Kind::Folder, _) if self.holds_refused => Message::new("sources.check.folderWithRefused"),
      (Kind::Folder, Some(1)) => Message::new("sources.check.folderOne"),
      (Kind::Folder, Some(files)) => Message::new("sources.check.folder").with("count", files.to_string()),
      (Kind::Folder, None) => Message::new("sources.check.largeFolder").with("count", COUNT_LIMIT.to_string()),
    }
  }
}

/// Why a path cannot become a source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
  Path(SourceError),
  NotFound,
  Unreadable,
  /// It lies in the project's own folder, which every run's worktree holds.
  InsideProject {
    project: String,
  },
  /// A source it would join holds it already.
  Covered(SourcePath),
  Git,
  OrchesterData,
  /// Another project's `.orchester` state, in the folder named `name`.
  OtherProject {
    name: String,
  },
  Credentials,
}

impl Refusal {
  pub fn message(&self) -> Message {
    match self {
      Self::Path(SourceError::Blank) => Message::new("sources.refused.blank"),
      Self::Path(SourceError::TooLong) => {
        Message::new("sources.refused.tooLong").with("max", SourcePath::MAX_CHARS.to_string())
      }
      Self::Path(SourceError::ControlCharacter) => Message::new("sources.refused.control"),
      Self::Path(SourceError::NotAbsolute(_)) => Message::new("sources.refused.notAbsolute"),
      Self::Path(SourceError::NotLocal(_)) => Message::new("sources.refused.notLocal"),
      Self::NotFound => Message::new("sources.refused.notFound"),
      Self::Unreadable => Message::new("sources.refused.unreadable"),
      Self::InsideProject { project } => Message::new("sources.refused.insideProject").with("project", project.clone()),
      Self::Covered(source) => Message::new("sources.refused.covered").with("source", source.as_str().to_owned()),
      Self::Git => Message::new("sources.refused.git"),
      Self::OrchesterData => Message::new("sources.refused.orchesterData"),
      Self::OtherProject { name } => Message::new("sources.refused.otherProject").with("name", name.clone()),
      Self::Credentials => Message::new("sources.refused.credentials"),
    }
  }

  /// Whether the path is wrong or refused, rather than not needed: a path inside the project or a source.
  pub const fn is_error(&self) -> bool {
    !matches!(self, Self::InsideProject { .. } | Self::Covered(_))
  }
}

/// Where a path is checked: the project's folder and name, and the sources it would join.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
  project_root: PathBuf,
  project: String,
  listed: Vec<SourcePath>,
}

impl Place {
  pub fn new(project_root: &Path, project: &str, listed: Vec<SourcePath>) -> Self {
    Self {
      project_root: project_root.to_path_buf(),
      project: project.to_owned(),
      listed,
    }
  }

  /// `typed` as a source, or why it cannot be one. Reads the file system: call it off the UI thread.
  pub fn check(&self, typed: &str) -> Result<Candidate, Refusal> {
    let path = SourcePath::new(typed).map_err(Refusal::Path)?;
    let (resolved, kind) = self.resolve(&path)?;
    if let Some(source) = self.covering(&resolved) {
      return Err(Refusal::Covered(source.clone()));
    }
    let (files, holds_refused) = match kind {
      Kind::File => {
        fs::File::open(&resolved).map_err(|error| os_refusal(&error))?;
        (None, false)
      }
      Kind::Folder => count(&resolved)?,
    };
    Ok(Candidate {
      path,
      kind,
      files,
      holds_refused,
    })
  }

  /// `path` resolved, with what it names, unless the reads outside a worktree refuse it or it lies in the project.
  fn resolve(&self, path: &SourcePath) -> Result<(PathBuf, Kind), Refusal> {
    let scope = CommandScope::on_host(&self.project_root, &[]);
    let project = scope.worktree().to_path_buf();
    let reads = OutsideReads::new(scope, &refused_folders());
    let (resolved, kind) = match reads.check(path.as_str()) {
      Ok(file) => (file, Kind::File),
      Err(OutsideReadError::NotAFile(_)) => (
        reads
          .check_folder(path.as_str())
          .map_err(|error| refusal(path, &error))?,
        Kind::Folder,
      ),
      Err(error) => return Err(refusal(path, &error)),
    };
    if resolved.starts_with(&project) {
      return Err(Refusal::InsideProject {
        project: self.project.clone(),
      });
    }
    Ok((resolved, kind))
  }

  /// The listed source that holds `resolved`, compared as each names a place on disk now.
  fn covering(&self, resolved: &Path) -> Option<&SourcePath> {
    self
      .listed
      .iter()
      .find(|source| ResolvedSources::resolve(&RunSources::union([(*source, SourceOrigin::Project)])).covers(resolved))
  }
}

/// The folders of Orchester's own data on this computer, which no read reaches: the application database's and log
/// folder, and every project's private files. A folder that cannot be told is left out: the others still hold.
fn refused_folders() -> Vec<PathBuf> {
  let mut folders = vec![std::env::temp_dir().join("orchester-mcp")];
  if let Ok(location) = orchester_store::location::app_data_location() {
    folders.extend(location.database.parent().map(Path::to_path_buf));
    folders.push(location.log_dir);
  }
  folders
}

fn refusal(path: &SourcePath, error: &OutsideReadError) -> Refusal {
  match error {
    OutsideReadError::Path(ScopeError::Unresolved { kind, .. }) if *kind == io::ErrorKind::NotFound => {
      Refusal::NotFound
    }
    OutsideReadError::Path(ScopeError::Unresolved { .. })
    | OutsideReadError::NotAFolder(_)
    | OutsideReadError::Unreadable { .. }
    | OutsideReadError::Binary(_) => Refusal::Unreadable,
    OutsideReadError::Path(_) | OutsideReadError::NotAFile(_) => {
      Refusal::Path(SourceError::NotLocal(path.as_str().to_owned()))
    }
    OutsideReadError::NotAbsolute(typed) => Refusal::Path(SourceError::NotAbsolute(typed.clone())),
    OutsideReadError::Credentials(_) => Refusal::Credentials,
    OutsideReadError::Protected(resolved) if in_git_folder(resolved) => Refusal::Git,
    OutsideReadError::Protected(resolved) => {
      state_owner(resolved).map_or(Refusal::OrchesterData, |name| Refusal::OtherProject { name })
    }
  }
}

/// The name of the folder whose `.orchester` state `resolved` is or lies in.
fn state_owner(resolved: &Path) -> Option<String> {
  let mut parent = None;
  for component in resolved.components() {
    if let Component::Normal(name) = component {
      if name.to_string_lossy().eq_ignore_ascii_case(STATE_DIR) {
        return parent.map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned());
      }
      parent = Some(name);
    }
  }
  None
}

/// A read the operating system refused.
fn os_refusal(error: &io::Error) -> Refusal {
  if error.kind() == io::ErrorKind::NotFound {
    Refusal::NotFound
  } else {
    Refusal::Unreadable
  }
}

/// The files of `folder`, up to [`COUNT_LIMIT`] entries walked (`None` past it), and whether it holds a `.git`
/// folder or Orchester state, which are not walked. Links are not followed.
fn count(folder: &Path) -> Result<(Option<usize>, bool), Refusal> {
  let first = fs::read_dir(folder).map_err(|error| os_refusal(&error))?;
  let (mut files, mut walked, mut holds_refused) = (0_usize, 0_usize, false);
  let mut pending = vec![first];
  while let Some(entries) = pending.pop() {
    for entry in entries.flatten() {
      walked += 1;
      if walked > COUNT_LIMIT {
        return Ok((None, holds_refused));
      }
      let name = entry.file_name().to_string_lossy().to_lowercase();
      if name == ".git" || name == STATE_DIR {
        holds_refused = true;
        continue;
      }
      match entry.file_type() {
        Ok(kind) if kind.is_dir() => {
          // A subfolder that cannot be read is not counted; the agent's reads say so there.
          if let Ok(inner) = fs::read_dir(entry.path()) {
            pending.push(inner);
          }
        }
        Ok(kind) if kind.is_file() => files += 1,
        _ => {}
      }
    }
  }
  Ok((Some(files), holds_refused))
}
