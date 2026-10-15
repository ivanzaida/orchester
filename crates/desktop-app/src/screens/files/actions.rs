//! *Copy* and *Open in editor*, a person's actions only: synthetic input (an MCP client driving the window)
//! pressing either is refused and the page says so, so automation never gets a file's text onto the
//! clipboard or a program started through the viewer. *Open in editor* opens the file on disk with the
//! system's default program, off the UI thread; the workspace decides what may be opened
//! ([`open_target`]).

use std::{path::PathBuf, sync::Arc, thread};

use lurq::{app::events::is_synthetic_input, core::Signal};
use orchester_exec::{external, programs::Programs};
use orchester_workspace::files::{FilePath, FileText, LineRange, OpenError, OpenWith, Version, open_target};

use super::Refusal;
use crate::ui::code_box::Copier;

/// Copies `lines` of `text`, or all of it, when a person asked.
pub fn copy(
  clipboard: Copier,
  refusal: Signal<Option<Refusal>>,
  (text, lines): (Arc<FileText>, Option<LineRange>),
) -> impl Fn() + Send + Sync + 'static {
  move || {
    if is_synthetic_input() {
      tracing::warn!("synthetic input pressed Copy in the code viewer; not copied");
      refusal.set(Some(Refusal::PersonOnly));
      return;
    }
    refusal.set(None);
    let copied = match lines {
      Some(lines) => text.copied(lines),
      None => text.lines().join("\n"),
    };
    if !clipboard.copy(&copied) {
      tracing::warn!("the clipboard did not take the code viewer's copy");
    }
  }
}

/// Opens `path` at `at` of the project at `root` with its default program, when a person asked.
pub fn open_in_editor(
  programs: Programs,
  refusal: Signal<Option<Refusal>>,
  (root, at, path): (PathBuf, Version, FilePath),
) -> impl Fn() + Send + Sync + 'static {
  move || {
    if is_synthetic_input() {
      tracing::warn!("synthetic input pressed Open in editor in the code viewer; not opened");
      refusal.set(Some(Refusal::PersonOnly));
      return;
    }
    refusal.set(None);
    let (programs, shown, root, at, path) = (
      programs.clone(),
      refusal.clone(),
      root.clone(),
      at.clone(),
      path.clone(),
    );
    let opening = thread::Builder::new().name("open-file".to_owned()).spawn(move || {
      let refused = match open_target(programs.runner(), &root, &at, &path) {
        Ok((file, with)) => launch(&file, with).err().map(|error| {
          tracing::warn!(%error, %path, "the system did not open the file the code viewer shows");
          Refusal::OpenFailed
        }),
        Err(error) => {
          tracing::warn!(%error, %path, "the code viewer does not open the file");
          Some(match error {
            OpenError::Refused(_, refusal) => Refusal::NotOpened(refusal),
            OpenError::NotOnDisk | OpenError::Files(_) | OpenError::Unsupported => Refusal::OpenFailed,
          })
        }
      };
      shown.set(refused);
    });
    if let Err(error) = opening {
      tracing::error!(%error, "cannot start opening a file from the code viewer");
      refusal.set(Some(Refusal::OpenFailed));
    }
  }
}

/// Opens `file` as the workspace decided it opens.
fn launch(file: &std::path::Path, with: OpenWith) -> std::io::Result<()> {
  match with {
    OpenWith::DefaultProgram => external::open_path(file),
    OpenWith::TextEditor => external::open_text(file),
  }
}

/// Why a file does not open, as the page says it.
pub const fn refusal_key(refusal: orchester_workspace::files::Refusal) -> &'static str {
  use orchester_workspace::files::Refusal;
  match refusal {
    Refusal::NotText => "files.openNotText",
    Refusal::WouldRun => "files.openRuns",
    Refusal::Link => "files.openLink",
    Refusal::Executable => "files.openExecutable",
  }
}
