//! Warm folders: the git-ignored folders a workspace keeps when it switches to another run, so build output and
//! downloaded packages stay. Each is a pattern relative to the repository, a folder (`target/`) or folders at one
//! level (`packages/*/node_modules/`), set to keep between runs or to clean every run (for folders whose leftovers
//! could fool tests, such as `dist/`). Everything else git ignores is cleaned on every switch.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::time::Timestamp;

/// How deep a pattern may name a folder: a person names folders deeper than suggestions go, but not without end.
const MAX_DEPTH: usize = 8;
/// The longest pattern, in bytes.
const MAX_BYTES: usize = 300;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WarmPatternError {
  #[error("a warm folder needs a name")]
  Empty,
  #[error("{0:?} is not inside the repository: write it relative to the repository's folder")]
  OutsideRepository(String),
  #[error("{0:?} has an empty, `.` or `..` part")]
  BadPart(String),
  #[error("{0:?} uses `*` inside a name or as its last part; `*` stands for a whole folder name between two others")]
  Wildcard(String),
  #[error("{0:?} names git's own folder")]
  GitFolder(String),
  #[error("{0:?} is more than {MAX_DEPTH} folders deep")]
  TooDeep(String),
  #[error("a warm folder is at most {MAX_BYTES} bytes long")]
  TooLong,
}

/// A folder pattern relative to the repository, with `/` between names and at its end: `target/`,
/// `packages/*/node_modules/`. `*` stands for one whole folder name and is never the last part.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WarmPattern(String);

impl WarmPattern {
  /// `text` as a pattern: `\` reads as `/`, a leading `./` is dropped and the closing `/` added.
  pub fn new(text: &str) -> Result<Self, WarmPatternError> {
    let text = text.trim().replace('\\', "/");
    let text = text.strip_prefix("./").unwrap_or(&text);
    if text.is_empty() || text == "/" {
      return Err(WarmPatternError::Empty);
    }
    if text.len() > MAX_BYTES {
      return Err(WarmPatternError::TooLong);
    }
    if text.starts_with('/') || text.contains(':') {
      return Err(WarmPatternError::OutsideRepository(text.to_owned()));
    }
    let trimmed = text.strip_suffix('/').unwrap_or(text);
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.iter().any(|part| part.is_empty() || *part == "." || *part == "..") {
      return Err(WarmPatternError::BadPart(text.to_owned()));
    }
    let partial = |part: &&str| part.contains(['*', '?', '[', ']']) && **part != *"*";
    if parts.iter().any(partial) || parts.last() == Some(&"*") {
      return Err(WarmPatternError::Wildcard(text.to_owned()));
    }
    if parts.iter().any(|part| part.eq_ignore_ascii_case(".git")) {
      return Err(WarmPatternError::GitFolder(text.to_owned()));
    }
    if parts.len() > MAX_DEPTH {
      return Err(WarmPatternError::TooDeep(text.to_owned()));
    }
    Ok(Self(format!("{trimmed}/")))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// Its folder names, without the slashes.
  pub fn parts(&self) -> impl Iterator<Item = &str> {
    self.0.trim_end_matches('/').split('/')
  }

  /// How many folders deep it names a folder: `target/` is 1, `packages/*/node_modules/` 3.
  pub fn depth(&self) -> usize {
    self.parts().count()
  }

  /// Whether `folder`, relative to the repository with `/` between names (a closing `/` or not), is a folder it
  /// names.
  pub fn matches(&self, folder: &str) -> bool {
    let folder = folder.trim_end_matches('/');
    let mut names = folder.split('/');
    let mut parts = self.parts();
    loop {
      match (parts.next(), names.next()) {
        (None, None) => return true,
        (Some(part), Some(name)) if part == "*" || part == name => {}
        _ => return false,
      }
    }
  }

  /// Whether `path`, relative to the repository, is a folder it names or lies inside one.
  pub fn holds(&self, path: &str) -> bool {
    let path = path.trim_end_matches('/');
    let depth = self.depth();
    let names: Vec<&str> = path.split('/').collect();
    names.len() >= depth && self.matches(&names[..depth].join("/"))
  }

  /// Whether a folder it names may lie inside `folder` (relative to the repository), deeper down: `build/` leads to
  /// `build/cache/`, and `packages/` to `packages/*/node_modules/`.
  pub fn lies_inside(&self, folder: &str) -> bool {
    let folder = folder.trim_end_matches('/');
    let names: Vec<&str> = folder.split('/').collect();
    let parts: Vec<&str> = self.parts().collect();
    names.len() < parts.len()
      && names
        .iter()
        .zip(&parts)
        .all(|(name, part)| *part == "*" || part == name)
  }
}

impl std::fmt::Display for WarmPattern {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for WarmPattern {
  type Error = WarmPatternError;

  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(&value)
  }
}

impl From<WarmPattern> for String {
  fn from(pattern: WarmPattern) -> Self {
    pattern.0
  }
}

/// What a switch does with a warm folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmChoice {
  /// Keep between runs: it survives switching to another run's branch.
  Keep,
  /// Clean every run: its leftovers could fool tests, so it goes like everything else ignored.
  Clean,
}

/// A warm folder of the project: its pattern, what a switch does with it, and when the person's checkout last had
/// it (`None` when it was never seen there, as for one a person added before building it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmFolder {
  pub pattern: WarmPattern,
  pub choice: WarmChoice,
  pub last_seen: Option<Timestamp>,
}

impl WarmFolder {
  pub const fn new(pattern: WarmPattern, choice: WarmChoice) -> Self {
    Self {
      pattern,
      choice,
      last_seen: None,
    }
  }
}

/// What a switch does with an ignored entry `path` (relative to the repository, a folder ending in `/`) as
/// `folders` say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cleaning {
  /// A folder kept between runs, or inside one.
  Keep,
  /// A folder that holds kept folders deeper down: only what lies beside them goes.
  Descend,
  Remove,
}

/// How a switch cleans the ignored entry `path`: only folders set to keep between runs stay.
pub fn cleaning(folders: &[WarmFolder], path: &str) -> Cleaning {
  let kept = || folders.iter().filter(|folder| folder.choice == WarmChoice::Keep);
  if kept().any(|folder| folder.pattern.holds(path)) {
    Cleaning::Keep
  } else if path.ends_with('/') && kept().any(|folder| folder.pattern.lies_inside(path)) {
    Cleaning::Descend
  } else {
    Cleaning::Remove
  }
}
