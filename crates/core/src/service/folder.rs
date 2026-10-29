use super::ServiceError;

/// The longest folder or path pattern, in characters.
const MAX_PATH_CHARS: usize = 1024;

/// Whether `value` stays inside the folder it is relative to: no drive (`C:`), no leading separator (a root or a
/// UNC share) and no `..` part. Both separators count on every platform, so a definition means the same on
/// Windows and macOS.
fn stays_inside(value: &str) -> bool {
  let mut chars = value.chars();
  let drive = chars.next().is_some_and(|letter| letter.is_ascii_alphabetic()) && chars.next() == Some(':');
  !drive
    && !value.starts_with(['/', '\\'])
    && !value.split(['/', '\\']).any(|part| part == "..")
    && !value.chars().any(char::is_control)
    && value.chars().count() <= MAX_PATH_CHARS
}

/// A folder inside the run's worktree, e.g. `web/team`; empty, `.` or `./` is the worktree itself. It is kept
/// with forward slashes and without empty or `.` parts, so `web\team` and `./web/team/` are the same folder.
/// Surrounding spaces are not part of it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RelativeFolder(String);

impl RelativeFolder {
  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let value = value.trim();
    let parts: Vec<&str> = value
      .split(['/', '\\'])
      .filter(|part| !part.is_empty() && *part != ".")
      .collect();
    // Windows drops a part's trailing dots and spaces, so `...` or `web.` would name another folder than written.
    let plain = parts.iter().all(|part| !part.ends_with(['.', ' ']));
    let folder = parts.join("/");
    if !plain || !stays_inside(value) || !stays_inside(&folder) {
      return Err(ServiceError::OutsideWorktree(value.to_owned()));
    }
    Ok(Self(folder))
  }

  /// The worktree itself.
  pub const fn root() -> Self {
    Self(String::new())
  }

  pub const fn is_root(&self) -> bool {
    self.0.is_empty()
  }

  /// The folder with forward slashes; empty for the worktree itself.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// The folder of a run's own files, below the worktree, e.g. `runs/{{run}}/team-web`. It is deleted when the run
/// ends, so it is never the worktree itself.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RunFolder(RelativeFolder);

impl RunFolder {
  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let folder = RelativeFolder::new(value)?;
    if folder.is_root() {
      return Err(ServiceError::RunFolderIsRoot);
    }
    Ok(Self(folder))
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

/// A repository path pattern a service owns, e.g. `web/cloud/**`: a run that changed a matching path starts the
/// service in its Test step.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnedPath(String);

impl OwnedPath {
  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let value = value.trim();
    if value.is_empty() || value == "." || !stays_inside(value) {
      return Err(ServiceError::OwnedPath(value.to_owned()));
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}
