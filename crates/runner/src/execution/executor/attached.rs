//! The copies of the files people attached to messages, as the executor gives them to its agents
//! (`orchester_core::attachment::input`): the coordinator link answers each file with its content
//! ([`super::link::messages`]), and the executor keeps it in its execution root's attachments folder under the name the
//! project's copy has. Locally that folder is the project's own and the copy is already there with that content, so
//! nothing is written and the agents get the very path they always got. A copy that is missing or holds something
//! else is written in its place (a temporary file renamed into place, so an agent never reads half of one).
//!
//! A file the coordinator could not read leaves its copy as it is: reading it fails where the agent's input is made,
//! as an unreadable copy always did there.

use std::{
  fs, io,
  path::Path,
  process,
  sync::atomic::{AtomicU64, Ordering},
};

use orchester_core::attachment::input::AttachedFile;
use thiserror::Error;

use super::link::{LinkError, messages::AttachedContent};

/// Tells apart the temporary files of copies written at once in this process.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// Why the files attached to messages could not be given to an agent.
#[derive(Debug, Error)]
pub(super) enum AttachedError {
  /// The link did not answer them.
  #[error(transparent)]
  Link(#[from] LinkError),
  /// A copy of one could not be kept.
  #[error("cannot keep a copy of an attached file: {0}")]
  Copy(#[from] io::Error),
}

/// The files `attached` as the agents are given them, each kept in `folder`.
pub(super) fn kept_in(folder: &Path, attached: Vec<AttachedContent>) -> io::Result<Vec<AttachedFile>> {
  attached
    .into_iter()
    .map(|file| {
      let name = file.attachment.file_name();
      match &file.content {
        Ok(content) => keep(folder, &name, content)?,
        Err(detail) => {
          tracing::warn!(attachment = %file.attachment.id(), %detail, "the project's copy of an attached file is unreadable");
        }
      }
      Ok(AttachedFile {
        attachment: file.attachment,
        path: folder.join(name),
      })
    })
    .collect()
}

/// Writes `content` to the copy `name` in `folder` unless it holds it already.
fn keep(folder: &Path, name: &str, content: &[u8]) -> io::Result<()> {
  let path = folder.join(name);
  match fs::read(&path) {
    Ok(kept) if kept == content => return Ok(()),
    Ok(_) => {}
    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
    Err(error) => return Err(error),
  }
  fs::create_dir_all(folder)?;
  let write = WRITES.fetch_add(1, Ordering::Relaxed);
  let temporary = folder.join(format!("{name}.{}-{write}.tmp", process::id()));
  fs::write(&temporary, content)?;
  fs::rename(&temporary, &path).inspect_err(|_| {
    // The rename's failure is what is reported; a temporary file left behind is only never read.
    let _ = fs::remove_file(&temporary);
  })
}
