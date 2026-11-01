//! Clones a repository in a console window the person sees (ORC-584, `docs/design/startup.md`, "Clone in a console"):
//! on Windows a PowerShell console, on macOS a Terminal window running a small script. Git runs there with the
//! person's own credentials and may ask them to sign in; Orchester never sees what is typed. The program, the address
//! and the folder reach Git as data, never as shell text, and Orchester learns only Git's exit code, which the console
//! writes to a result file of its own once Git ends. A window closed before Git ends leaves no result: the clone was
//! closed. Only this module's own scratch folder is ever removed; the clone's folder is never touched.

#[cfg(target_os = "macos")]
mod macos;
mod script;
#[cfg(windows)]
pub mod windows;

use std::{
  fs, io,
  path::{Path, PathBuf},
  thread,
  time::Duration,
};

use tempfile::TempDir;

pub use self::script::terminal_script;

/// How often the console's result and its window are looked at.
const POLL: Duration = Duration::from_millis(250);

/// The clone a console runs.
#[derive(Clone, Copy, Debug)]
pub struct CloneJob<'a> {
  /// The git program, found before the console opens.
  pub git: &'a Path,
  /// The repository's address, `https://…`, with no user info in it.
  pub url: &'a str,
  /// The folder Git creates for the checkout, or an empty one it clones into.
  pub destination: &'a Path,
  /// A folder for the console's own files: its script and its result.
  pub scratch: &'a Path,
  pub words: &'a ConsoleWords,
}

/// What the console window says itself: its title, and after a failure what happened and how to close it. Control
/// characters are dropped, so a project's name from a server can't send the terminal escape sequences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleWords {
  title: String,
  failed: String,
  close: String,
}

impl ConsoleWords {
  pub fn new(title: &str, failed: &str, close: &str) -> Self {
    let plain = |text: &str| text.chars().filter(|glyph| !glyph.is_control()).collect();
    Self {
      title: plain(title),
      failed: plain(failed),
      close: plain(close),
    }
  }

  pub fn title(&self) -> &str {
    &self.title
  }

  pub fn failed(&self) -> &str {
    &self.failed
  }

  pub fn close(&self) -> &str {
    &self.close
  }
}

/// How the console's clone ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleEnd {
  /// Git ended with `code`; 0 when it says the clone is done.
  Exited { code: i32 },
  /// The window closed, or the clone was interrupted, before Git ended.
  Closed,
}

/// Why the console's clone can't be followed.
#[derive(Debug, thiserror::Error)]
pub enum ConsoleError {
  #[error("the console window did not open")]
  NotOpened(#[source] io::Error),
  /// Terminal opened but never started the script (macOS).
  #[error("the terminal did not start the clone")]
  NotStarted,
  #[error("the console window could not be followed")]
  Wait(#[source] io::Error),
  /// The console wrote a result that is not an exit code.
  #[error("the console's result could not be read")]
  UnreadableResult,
  #[error("cloning in a console is not supported on this platform yet")]
  Unsupported,
}

/// A console window running a clone.
#[derive(Debug)]
pub struct CloneConsole {
  running: platform::Running,
  files: Scratch,
}

impl CloneConsole {
  /// Opens the window.
  pub fn open(job: &CloneJob<'_>) -> Result<Self, ConsoleError> {
    let files = Scratch::new(job.scratch).map_err(ConsoleError::NotOpened)?;
    let running = platform::open(job, &files)?;
    Ok(Self { running, files })
  }

  /// Blocks until Git ended or the window closed.
  pub fn wait(self) -> Result<ConsoleEnd, ConsoleError> {
    let Self { running, files } = self;
    let end = platform::wait(running, &files);
    files.remove();
    end
  }
}

/// The console's own files, in a folder of their own in the scratch folder.
#[derive(Debug)]
struct Scratch(TempDir);

impl Scratch {
  fn new(scratch: &Path) -> io::Result<Self> {
    fs::create_dir_all(scratch)?;
    tempfile::Builder::new().prefix("clone-").tempdir_in(scratch).map(Self)
  }

  /// Where the console writes Git's exit code.
  fn result(&self) -> PathBuf {
    self.file("result")
  }

  fn file(&self, name: &str) -> PathBuf {
    self.0.path().join(name)
  }

  /// Removes the folder: only the console's script, its result and its pid file are in it.
  fn remove(self) {
    let folder = self.0.path().to_owned();
    if let Err(error) = self.0.close() {
      // The clone's end is known; a scratch folder left behind only takes a little room in Orchester's own data.
      tracing::warn!(%error, folder = %folder.display(), "cannot remove a clone console's scratch folder");
    }
  }
}

/// One look at a console: its `result` first, then whether its host still runs, then the result once more, since Git
/// may have ended between the two. `None` while it runs.
pub fn read_end(
  result: &Path,
  host_running: impl FnOnce() -> Result<bool, ConsoleError>,
) -> Result<Option<ConsoleEnd>, ConsoleError> {
  if let Some(code) = read_code(result)? {
    return Ok(Some(ConsoleEnd::Exited { code }));
  }
  if host_running()? {
    return Ok(None);
  }
  Ok(Some(
    read_code(result)?.map_or(ConsoleEnd::Closed, |code| ConsoleEnd::Exited { code }),
  ))
}

/// The exit code in `result`; `None` while there is no result. The console writes it whole beside it and renames it,
/// so a result read is never half written.
fn read_code(result: &Path) -> Result<Option<i32>, ConsoleError> {
  match fs::read_to_string(result) {
    Ok(text) => text
      .trim()
      .parse()
      .map(Some)
      .map_err(|_| ConsoleError::UnreadableResult),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(ConsoleError::Wait(error)),
  }
}

/// Looks at the console every [`POLL`] until it ended.
fn follow(
  result: &Path,
  mut host_running: impl FnMut() -> Result<bool, ConsoleError>,
) -> Result<ConsoleEnd, ConsoleError> {
  loop {
    if let Some(end) = read_end(result, &mut host_running)? {
      return Ok(end);
    }
    thread::sleep(POLL);
  }
}

#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(windows)]
use self::windows as platform;

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
  use super::{CloneJob, ConsoleEnd, ConsoleError, Scratch};

  #[derive(Debug)]
  pub struct Running;

  pub fn open(_job: &CloneJob<'_>, _files: &Scratch) -> Result<Running, ConsoleError> {
    Err(ConsoleError::Unsupported)
  }

  pub fn wait(_running: Running, _files: &Scratch) -> Result<ConsoleEnd, ConsoleError> {
    Err(ConsoleError::Unsupported)
  }
}
