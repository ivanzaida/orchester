//! Files a person attached to a message (`orchester_core::attachment`): a copy of each in
//! `<repo>/.orchester/attachments/<id>.<extension>` and an `attachments` row that links it to its message, with its
//! name, kind, size, hash and an image's size. The copy is written under a temporary name and renamed into place
//! before its row is kept; a message whose rows are not kept leaves no copy behind. An attachment lives as long as its
//! message: a chat message the runner never took is forgotten with its copies.

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process,
};

use orchester_core::{
  attachment::{Attachment, AttachmentName, Dimensions, MediaType, NewAttachment, input::AttachedFile},
  blob::ContentHash,
  id::{AttachmentId, ChatMessageId, MessageId, TaskId},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{ProjectDb, blobs::BlobHash};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const FOLDER: &str = "attachments";
const COLUMNS: &str = "id, name, media_type, size, sha256, width, height";

/// The message attachments belong to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentOwner {
  /// A message of a task's conversation.
  Message(MessageId),
  /// A person's message in the Orchestrator chat.
  ChatMessage(ChatMessageId),
}

impl AttachmentOwner {
  const fn column(self) -> &'static str {
    match self {
      Self::Message(_) => "message_id",
      Self::ChatMessage(_) => "chat_message_id",
    }
  }

  fn id(self) -> String {
    match self {
      Self::Message(id) => id.to_string(),
      Self::ChatMessage(id) => id.to_string(),
    }
  }
}

/// The copies of attachments written for a message whose rows are not kept yet: removed when dropped, unless the
/// message was kept ([`Self::keep`]).
pub(super) struct Written {
  paths: Vec<PathBuf>,
}

impl Written {
  /// The message was kept: its copies stay.
  pub(super) fn keep(mut self) {
    self.paths.clear();
  }
}

impl Drop for Written {
  fn drop(&mut self) {
    for path in &self.paths {
      remove(path);
    }
  }
}

/// Where the project at `repo_root` keeps the copy of `attachment`.
pub fn attachment_file(repo_root: &Path, attachment: &Attachment) -> PathBuf {
  path(&repo_root.join(super::DIRECTORY), attachment)
}

impl ProjectDb {
  /// Where the project keeps the copy of `attachment`.
  pub fn attachment_path(&self, attachment: &Attachment) -> PathBuf {
    path(&self.directory, attachment)
  }

  /// The attachments of each message of `task` that has some, in the order they were attached.
  pub fn message_attachments(&self, task: TaskId) -> Result<BTreeMap<MessageId, Vec<Attachment>>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT message_id, {COLUMNS} FROM attachments WHERE message_id IN (SELECT id FROM messages WHERE task_id = ?1)
       ORDER BY message_id, position"
    ))?;
    let rows = statement.query_map([task.to_string()], |row| Ok((row.get::<_, String>(0)?, raw(row, 1)?)))?;
    let mut owned: BTreeMap<MessageId, Vec<Attachment>> = BTreeMap::new();
    for row in rows {
      let (message, attachment) = row?;
      owned
        .entry(parse("message id", &message)?)
        .or_default()
        .push(attachment.into_attachment()?);
    }
    Ok(owned)
  }

  /// The attachments of each person's message of chat conversation `chat` that has some.
  pub fn chat_attachments(
    &self,
    chat: orchester_core::id::ChatId,
  ) -> Result<BTreeMap<ChatMessageId, Vec<Attachment>>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT chat_message_id, {COLUMNS} FROM attachments
       WHERE chat_message_id IN (SELECT message_id FROM orchestrator_entries WHERE chat_id = ?1)
       ORDER BY chat_message_id, position"
    ))?;
    let rows = statement.query_map([chat.to_string()], |row| Ok((row.get::<_, String>(0)?, raw(row, 1)?)))?;
    let mut owned: BTreeMap<ChatMessageId, Vec<Attachment>> = BTreeMap::new();
    for row in rows {
      let (message, attachment) = row?;
      owned
        .entry(parse("chat message id", &message)?)
        .or_default()
        .push(attachment.into_attachment()?);
    }
    Ok(owned)
  }

  /// The attachments of `owner`, in the order they were attached.
  pub fn attachments_of(&self, owner: AttachmentOwner) -> Result<Vec<Attachment>, StoreError> {
    attachments_of(&self.connection, owner)
  }

  /// The attachments of `owner` with the project's copies, as an agent is given them.
  pub fn attached_files(&self, owner: AttachmentOwner) -> Result<Vec<AttachedFile>, StoreError> {
    Ok(
      self
        .attachments_of(owner)?
        .into_iter()
        .map(|attachment| AttachedFile {
          path: path(&self.directory, &attachment),
          attachment,
        })
        .collect(),
    )
  }

  /// The attachment `id` with the project's copy; `None` when no message has it.
  pub fn attachment(&self, id: AttachmentId) -> Result<Option<AttachedFile>, StoreError> {
    let raw = self
      .connection
      .query_row(
        &format!("SELECT {COLUMNS} FROM attachments WHERE id = ?1"),
        [id.to_string()],
        |row| raw(row, 0),
      )
      .optional()?;
    raw
      .map(|raw| {
        let attachment = raw.into_attachment()?;
        Ok(AttachedFile {
          path: path(&self.directory, &attachment),
          attachment,
        })
      })
      .transpose()
  }
}

/// Writes the copies of `attachments` for a message about to be kept; they are removed again unless the message is.
pub(super) fn write(orchester: &Path, attachments: &[NewAttachment]) -> Result<(Written, Vec<Attachment>), StoreError> {
  let folder = orchester.join(FOLDER);
  if !attachments.is_empty() {
    fs::create_dir_all(&folder).map_err(StoreError::io(&folder))?;
  }
  let mut written = Written { paths: Vec::new() };
  let mut kept = Vec::with_capacity(attachments.len());
  for new in attachments {
    let hash = ContentHash::new(BlobHash::of(new.content()).as_str()).map_err(|error| corrupt("attachment hash", error))?;
    let attachment = Attachment::kept(new, hash);
    let target = folder.join(attachment.file_name());
    let temporary = folder.join(format!("{}.{}.tmp", attachment.file_name(), process::id()));
    fs::write(&temporary, new.content().as_ref()).map_err(StoreError::io(&temporary))?;
    if let Err(source) = fs::rename(&temporary, &target) {
      remove(&temporary);
      return Err(StoreError::Io { path: target, source });
    }
    written.paths.push(target);
    kept.push(attachment);
  }
  Ok((written, kept))
}

/// Keeps the rows of `attachments`, whose copies are written, for `owner` at `at`.
pub(super) fn insert(
  connection: &Connection,
  owner: AttachmentOwner,
  attachments: &[Attachment],
  at: Timestamp,
) -> Result<(), StoreError> {
  let sql = format!(
    "INSERT INTO attachments (id, {}, position, name, media_type, size, sha256, width, height, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    owner.column()
  );
  for (position, attachment) in attachments.iter().enumerate() {
    let size = i64::try_from(attachment.size()).map_err(|_| StoreError::Unrepresentable("attachment size"))?;
    let position = i64::try_from(position).map_err(|_| StoreError::Unrepresentable("attachment position"))?;
    let dimensions = attachment.dimensions();
    connection.execute(
      &sql,
      params![
        attachment.id().to_string(),
        owner.id(),
        position,
        attachment.name().as_str(),
        attachment.media().as_str(),
        size,
        attachment.hash().as_str(),
        dimensions.map(|size| size.width),
        dimensions.map(|size| size.height),
        at.unix_millis()
      ],
    )?;
  }
  Ok(())
}

/// Removes the rows of `owner`'s attachments; the paths of their copies, to remove once that is committed.
pub(super) fn forget(
  connection: &Connection,
  orchester: &Path,
  owner: AttachmentOwner,
) -> Result<Vec<PathBuf>, StoreError> {
  let paths = attachments_of(connection, owner)?
    .iter()
    .map(|attachment| path(orchester, attachment))
    .collect();
  connection.execute(
    &format!("DELETE FROM attachments WHERE {} = ?1", owner.column()),
    [owner.id()],
  )?;
  Ok(paths)
}

/// Removes the copies of attachments whose rows are gone. A copy left behind is never read again, only wastes space,
/// and the change it belonged to is done; so a failed removal is logged by no one and dropped.
pub(super) fn remove_copies(paths: &[PathBuf]) {
  for path in paths {
    remove(path);
  }
}

fn remove(path: &Path) {
  // See `remove_copies`: losing this error leaves only an unread file.
  let _ = fs::remove_file(path);
}

fn attachments_of(connection: &Connection, owner: AttachmentOwner) -> Result<Vec<Attachment>, StoreError> {
  let mut statement = connection.prepare(&format!(
    "SELECT {COLUMNS} FROM attachments WHERE {} = ?1 ORDER BY position",
    owner.column()
  ))?;
  let rows = statement
    .query_map([owner.id()], |row| raw(row, 0))?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(RawAttachment::into_attachment).collect()
}

fn path(orchester: &Path, attachment: &Attachment) -> PathBuf {
  orchester.join(FOLDER).join(attachment.file_name())
}

/// One stored row, from column `first` on, before it is checked.
struct RawAttachment {
  id: String,
  name: String,
  media: String,
  size: i64,
  hash: String,
  width: Option<u32>,
  height: Option<u32>,
}

fn raw(row: &Row<'_>, first: usize) -> rusqlite::Result<RawAttachment> {
  Ok(RawAttachment {
    id: row.get(first)?,
    name: row.get(first + 1)?,
    media: row.get(first + 2)?,
    size: row.get(first + 3)?,
    hash: row.get(first + 4)?,
    width: row.get(first + 5)?,
    height: row.get(first + 6)?,
  })
}

impl RawAttachment {
  fn into_attachment(self) -> Result<Attachment, StoreError> {
    let name = AttachmentName::new(&self.name).ok_or_else(|| corrupt("attachment name", &self.name))?;
    let media = MediaType::parse(&self.media).ok_or_else(|| corrupt("attachment media type", &self.media))?;
    let size = u64::try_from(self.size).map_err(|error| corrupt("attachment size", error))?;
    let hash = ContentHash::new(&self.hash).map_err(|error| corrupt("attachment hash", error))?;
    let dimensions = match (self.width, self.height) {
      (Some(width), Some(height)) => Some(Dimensions { width, height }),
      _ => None,
    };
    Ok(Attachment::restore(
      parse("attachment id", &self.id)?,
      (name, media),
      (size, hash),
      dimensions,
    ))
  }
}
