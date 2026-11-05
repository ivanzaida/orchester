//! Why the preflight of a local merge refused before Git ran: the stage that refused and its cause, as a merge
//! attempt's *What stopped it* tells them (`crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09b
//! and 09e). A cause holds what the OS or Git said and a path relative to the project, never a secret value: Orchester
//! gives none to these Git programs, and a URL's user part in Git's words is replaced.

use std::{
  fmt, io,
  path::{Path, PathBuf},
};

use super::local::LocalError;
use crate::repository::redact_userinfo;

/// The last lines of Git's error output a cause keeps, and the most characters of them.
const GIT_LINES: usize = 4;
const GIT_CHARS: usize = 600;

/// The step of a merge's preflight that refused it; nothing was merged and the branch and checkout are unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightStage {
  /// The project's integration lease, `.orchester/local-integration.lock`.
  IntegrationLock,
  /// The project database and SQLite's files beside it.
  ProjectDatabase,
  /// Local integration's scratch folder under `.orchester`.
  ScratchFolder,
  /// The checkout and its Git directory, as Git names them.
  CheckoutIdentity,
  /// The repository's Git metadata, held while Git changes it.
  GitMetadata,
  /// Git's evidence of the run's work merged already.
  ExistingResult,
  /// The policy an automation's merge goes through.
  Policy,
}

impl PreflightStage {
  /// The stage's name in logs and in an MCP client's reasons.
  pub const fn name(self) -> &'static str {
    match self {
      Self::IntegrationLock => "integration_lock",
      Self::ProjectDatabase => "project_database",
      Self::ScratchFolder => "scratch_folder",
      Self::CheckoutIdentity => "checkout_identity",
      Self::GitMetadata => "git_metadata",
      Self::ExistingResult => "existing_result",
      Self::Policy => "policy",
    }
  }
}

/// An operating system error, with the path it concerned when one is known, as the OS reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoCause {
  path: Option<PathBuf>,
  kind: io::ErrorKind,
  code: Option<i32>,
  text: String,
}

impl IoCause {
  pub fn new(error: &io::Error) -> Self {
    let text = error.to_string();
    // std appends " (os error N)" to the OS's own words; the code is kept apart.
    let text = match error.raw_os_error() {
      Some(code) => text
        .strip_suffix(&format!(" (os error {code})"))
        .unwrap_or(&text)
        .to_owned(),
      None => text,
    };
    Self {
      path: None,
      kind: error.kind(),
      code: error.raw_os_error(),
      text,
    }
  }

  /// What Orchester found itself, without an OS error: e.g. a folder that is not there.
  pub fn of_kind(kind: io::ErrorKind) -> Self {
    Self::new(&io::Error::from(kind))
  }

  /// This cause, about `path` unless it names a path already.
  #[must_use]
  pub fn at(mut self, path: &Path) -> Self {
    self.path.get_or_insert_with(|| path.to_owned());
    self
  }

  pub fn path(&self) -> Option<&Path> {
    self.path.as_deref()
  }

  pub const fn kind(&self) -> io::ErrorKind {
    self.kind
  }

  /// The OS's own error number, when the OS reported the error.
  pub const fn code(&self) -> Option<i32> {
    self.code
  }

  pub fn text(&self) -> &str {
    &self.text
  }
}

/// How Git ended when it refused: its exit code and the last lines of its error output, bounded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitCause {
  exit: Option<i32>,
  tail: String,
}

impl GitCause {
  /// Git's exit code (`None`: ended by a signal) and the end of what it wrote to its error output.
  pub fn new(exit: Option<i32>, stderr: &[u8]) -> Self {
    let text = redact_userinfo(&String::from_utf8_lossy(stderr));
    let lines: Vec<&str> = text
      .lines()
      .map(str::trim_end)
      .filter(|line| !line.is_empty())
      .collect();
    let tail = lines[lines.len().saturating_sub(GIT_LINES)..].join("\n");
    let skip = tail.chars().count().saturating_sub(GIT_CHARS);
    Self {
      exit,
      tail: tail.chars().skip(skip).collect(),
    }
  }

  pub const fn exit(&self) -> Option<i32> {
    self.exit
  }

  /// The last lines Git wrote to its error output.
  pub fn tail(&self) -> &str {
    &self.tail
  }
}

/// Why a stage of the preflight refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreflightCause {
  /// Another merge or local integration holds the project's integration lease.
  Held,
  /// The OS refused; `path` is relative to the project, `None` when the error named none or one outside it.
  Io {
    path: Option<String>,
    kind: io::ErrorKind,
    code: Option<i32>,
    text: String,
  },
  /// Git refused.
  Git(GitCause),
  /// The checkout or a file in it is not as local integration requires.
  State(LocalError),
  /// Orchester's own reason, in its fixed words.
  Reason(String),
}

impl PreflightCause {
  /// `cause`, its path relative to the project at `root`.
  pub fn io(cause: &IoCause, root: &Path) -> Self {
    Self::Io {
      path: cause.path().and_then(|path| relative(path, root)),
      kind: cause.kind(),
      code: cause.code(),
      text: cause.text().to_owned(),
    }
  }
}

/// `path` relative to `root`, with `/` between names; `None` outside it. The root is tried as given and resolved, since
/// a path opened through a link (macOS's `/var`) is named by the folder it reaches.
fn relative(path: &Path, root: &Path) -> Option<String> {
  let inside = path
    .strip_prefix(root)
    .ok()
    .map(Path::to_owned)
    .or_else(|| Some(path.strip_prefix(root.canonicalize().ok()?).ok()?.to_owned()))?;
  let name = inside.to_string_lossy().replace('\\', "/");
  Some(if name.is_empty() { ".".to_owned() } else { name })
}

impl fmt::Display for PreflightCause {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Held => f.write_str("held by another operation"),
      Self::Io { path, code, text, .. } => {
        if let Some(path) = path {
          write!(f, "{path}: ")?;
        }
        f.write_str(text)?;
        match code {
          Some(code) => write!(f, " (OS error {code})"),
          None => Ok(()),
        }
      }
      Self::Git(cause) => {
        match cause.exit() {
          Some(code) => write!(f, "git exited with {code}")?,
          None => f.write_str("git was stopped")?,
        }
        match cause.tail() {
          "" => Ok(()),
          tail => write!(f, ": {tail}"),
        }
      }
      Self::State(error) => write!(f, "{error}"),
      Self::Reason(reason) => f.write_str(reason),
    }
  }
}
