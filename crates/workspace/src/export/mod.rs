//! *Export conversation…* (ORC-323, ORC-327, `docs/design/copy-and-export.md`): an Orchestrator conversation
//! ([`chat`]) or a run's Conversation ([`run`]) as one Markdown file, with or without its tool calls ([`Include`]). It
//! holds a title, the date it was exported, the conversation or the task and run, each message under who wrote it and
//! when, and for a run a heading for each step. Attachments are named, never embedded. The text is the kept one, which
//! the runner redacted as it came, and the values of the project's secrets are masked in it once more ([`screen_in`])
//! before it is copied or written. Writing ([`write_to`]) says why it failed, and only a file that was written counts.
//! The app picks the file; automation could write the same text.

pub mod chat;
mod document;
pub mod run;

use std::{
  io::ErrorKind,
  path::Path,
  time::{Duration, UNIX_EPOCH},
};

use orchester_core::{memory::SecretScreen, secret::SecretVault, store::SecretStore, time::Timestamp};
use orchester_store::ProjectDb;

use crate::messages::{self, Message};

/// What the file holds besides the messages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Include {
  /// The messages with their code blocks.
  #[default]
  Messages,
  /// The messages and each tool call with its input and a short result.
  WithTools,
}

/// How the file's own words (who wrote a message, its facts' labels) read in the person's language.
pub type Words<'a> = &'a dyn Fn(&Message) -> String;

/// The local date and time of `at` as the file says it: `2026-11-03 16:20`.
pub fn stamp(at: Timestamp) -> String {
  // Stored times are after 1970; an earlier one reads as the epoch rather than failing.
  let millis = u64::try_from(at.unix_millis()).unwrap_or_default();
  chrono::DateTime::<chrono::Local>::from(UNIX_EPOCH + Duration::from_millis(millis))
    .format("%Y-%m-%d %H:%M")
    .to_string()
}

/// The file's name for `title`: its letters and digits in lower case, words joined by hyphens, and `.md`
/// (`brief-for-per-endpoint-retries.md`); `conversation.md` for a title without any.
pub fn file_name(title: &str) -> String {
  let words: Vec<String> = title
    .split(|c: char| !c.is_alphanumeric())
    .filter(|word| !word.is_empty())
    .map(str::to_lowercase)
    .collect();
  let slug = words.join("-");
  let slug: String = slug.chars().take(FILE_NAME_CHARS).collect();
  let slug = slug.trim_end_matches('-');
  let slug = if slug.is_empty() { UNTITLED } else { slug };
  format!("{slug}.md")
}

/// Characters of a title its file's name keeps.
const FILE_NAME_CHARS: usize = 80;
/// The file's name, without `.md`, for a title of no letters or digits.
const UNTITLED: &str = "conversation";

/// How big the file is, as the dialog says it (*≈ 9 KB*): whole kilobytes, at least one.
pub fn size(bytes: usize) -> Message {
  let kilobytes = bytes.div_ceil(1000).max(1);
  Message::new("export.size").with("count", kilobytes.to_string())
}

/// What masks the values of the secrets of the project at `root`, read from `vault`. An export that cannot read them
/// cannot keep them out and is refused. Blocks on SQLite and the keychain.
pub fn screen_in(root: &Path, vault: &dyn SecretVault) -> Result<SecretScreen, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let secrets = db.secrets().map_err(|error| messages::store_error(&error))?;
  SecretScreen::load(db.project().id(), &secrets, vault).map_err(|error| {
    tracing::warn!(secret = %error.secret, "a secret could not be read to mask it in an export");
    Message::new("export.errors.secretsUnreadable")
  })
}

/// Writes `markdown` to the file at `path`, in place of what it held; why not, in the person's words, when it could
/// not be written whole. Blocks on the file system.
pub fn write_to(path: &Path, markdown: &str) -> Result<(), Message> {
  std::fs::write(path, markdown).map_err(|error| {
    tracing::warn!(%error, path = %path.display(), "the conversation could not be exported");
    let key = match error.kind() {
      ErrorKind::PermissionDenied => "export.errors.denied",
      ErrorKind::NotFound => "export.errors.noFolder",
      ErrorKind::StorageFull | ErrorKind::QuotaExceeded => "export.errors.full",
      ErrorKind::ReadOnlyFilesystem => "export.errors.readOnly",
      ErrorKind::IsADirectory => "export.errors.folder",
      _ => "export.errors.failed",
    };
    Message::new(key).with("path", path.display().to_string())
  })
}
