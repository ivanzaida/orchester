//! Files a person attaches to a message, in the Orchestrator chat and in a task's conversation, as Claude Desktop
//! takes them: images (PNG, JPEG, GIF, WebP), PDFs and UTF-8 text, told apart by their content rather than their
//! name ([`detect`]); any other file, a folder or a shortcut is refused with why ([`AttachmentRefusal`]). A message
//! holds at most [`limits::MAX_ATTACHMENTS`], each up to [`limits::MAX_ATTACHMENT_BYTES`] and together up to
//! [`limits::MAX_MESSAGE_BYTES`]. The project keeps a copy of each file, never a path to the person's own, for as long
//! as its message lives (`orchester_store`); an agent gets it as its connection takes that kind ([`support`],
//! [`input`]). Attachments are data only: their content never reaches a log, a `Debug` output or an error.

mod detect;
pub mod input;
pub mod limits;
pub mod support;

use std::{fmt, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::detect::detect;
use crate::{blob::ContentHash, connection::AgentProvider, id::AttachmentId};

/// What a file is, as its content says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaType {
  #[serde(rename = "image/png")]
  Png,
  #[serde(rename = "image/jpeg")]
  Jpeg,
  #[serde(rename = "image/gif")]
  Gif,
  #[serde(rename = "image/webp")]
  Webp,
  #[serde(rename = "application/pdf")]
  Pdf,
  /// UTF-8 text: prose, data or source code of any kind.
  #[serde(rename = "text/plain")]
  Text,
}

/// The kinds a connection takes or refuses as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttachmentKind {
  Image,
  Pdf,
  Text,
}

impl MediaType {
  pub const ALL: [Self; 6] = [Self::Png, Self::Jpeg, Self::Gif, Self::Webp, Self::Pdf, Self::Text];

  /// The media type as MIME names it.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Png => "image/png",
      Self::Jpeg => "image/jpeg",
      Self::Gif => "image/gif",
      Self::Webp => "image/webp",
      Self::Pdf => "application/pdf",
      Self::Text => "text/plain",
    }
  }

  /// The media type MIME names `name`; `None` for one Orchester does not keep.
  pub fn parse(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|media| media.as_str() == name)
  }

  pub const fn kind(self) -> AttachmentKind {
    match self {
      Self::Png | Self::Jpeg | Self::Gif | Self::Webp => AttachmentKind::Image,
      Self::Pdf => AttachmentKind::Pdf,
      Self::Text => AttachmentKind::Text,
    }
  }

  /// The extension the project's copy is kept under, so programs that read it by name know what it is.
  pub const fn extension(self) -> &'static str {
    match self {
      Self::Png => "png",
      Self::Jpeg => "jpg",
      Self::Gif => "gif",
      Self::Webp => "webp",
      Self::Pdf => "pdf",
      Self::Text => "txt",
    }
  }
}

/// An image's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dimensions {
  pub width: u32,
  pub height: u32,
}

/// Why a file is not attached, or a message with attachments not sent.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AttachmentRefusal {
  #[error("the file is empty")]
  Empty,
  #[error("the file is larger than {limit} bytes")]
  TooLarge { limit: u64 },
  #[error("this kind of file is not supported yet")]
  Unsupported,
  #[error("a folder cannot be attached")]
  Folder,
  #[error("a shortcut cannot be attached")]
  Shortcut,
  #[error("the file cannot be read")]
  Unreadable,
  #[error("a message holds at most {limit} attachments")]
  TooMany { limit: usize },
  #[error("a message's attachments are larger than {limit} bytes together")]
  TotalTooLarge { limit: u64 },
  #[error("the agent's connection does not take this kind of file")]
  NotTaken { kind: AttachmentKind, by: AgentProvider },
}

/// The name a file is shown and given to agents under: its file name without any folder, never blank.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AttachmentName(String);

/// The most characters a name keeps.
const NAME_CHARS: usize = 255;

impl AttachmentName {
  /// `name` as a file's name: what follows its last folder separator, without control characters, trimmed and cut
  /// to [`NAME_CHARS`] characters; `None` when nothing is left.
  pub fn new(name: &str) -> Option<Self> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let clean: String = last.chars().filter(|c| !c.is_control()).take(NAME_CHARS).collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| Self(clean.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for AttachmentName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.0, f)
  }
}

impl TryFrom<String> for AttachmentName {
  type Error = String;

  fn try_from(value: String) -> Result<Self, String> {
    Self::new(&value).ok_or(value)
  }
}

impl From<AttachmentName> for String {
  fn from(name: AttachmentName) -> Self {
    name.0
  }
}

/// A file a person picked, pasted or dropped, read and checked, before the project keeps it with its message. Its
/// content is shared, not copied, between a draft and its sends; `Debug` shows its name, kind and size only.
#[derive(Clone)]
pub struct NewAttachment {
  id: AttachmentId,
  name: AttachmentName,
  media: MediaType,
  dimensions: Option<Dimensions>,
  content: Arc<[u8]>,
}

impl NewAttachment {
  /// `content` under `name`, as [`detect`] tells what it is; refused when it is empty, larger than
  /// [`limits::MAX_ATTACHMENT_BYTES`] or of a kind Orchester does not take.
  pub fn new(name: AttachmentName, content: impl Into<Arc<[u8]>>) -> Result<Self, AttachmentRefusal> {
    let content: Arc<[u8]> = content.into();
    limits::check_size(content.len() as u64)?;
    let (media, dimensions) = detect(&content)?;
    Ok(Self {
      id: AttachmentId::generate(),
      name,
      media,
      dimensions,
      content,
    })
  }

  pub const fn id(&self) -> AttachmentId {
    self.id
  }

  pub const fn name(&self) -> &AttachmentName {
    &self.name
  }

  pub const fn media(&self) -> MediaType {
    self.media
  }

  pub const fn dimensions(&self) -> Option<Dimensions> {
    self.dimensions
  }

  pub fn size(&self) -> u64 {
    self.content.len() as u64
  }

  pub fn content(&self) -> &Arc<[u8]> {
    &self.content
  }

  /// The text of a text file; `None` for an image or a PDF.
  pub fn text(&self) -> Option<&str> {
    match self.media {
      MediaType::Text => std::str::from_utf8(&self.content).ok(),
      _ => None,
    }
  }
}

impl PartialEq for NewAttachment {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Eq for NewAttachment {}

impl fmt::Debug for NewAttachment {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("NewAttachment")
      .field("id", &self.id)
      .field("name", &self.name)
      .field("media", &self.media)
      .field("size", &self.content.len())
      .finish()
  }
}

/// An attachment the project keeps with its message: what it is, never its content.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Attachment {
  id: AttachmentId,
  name: AttachmentName,
  media: MediaType,
  size: u64,
  /// The SHA-256 of its content, which the store computes when it keeps it.
  hash: ContentHash,
  dimensions: Option<Dimensions>,
}

impl Attachment {
  /// `new` as kept with one message, under the hash of its content. It gets an id of its own, not the draft's: the
  /// same draft file sent again is kept again for its new message, so no message's copy is another's.
  pub fn kept(new: &NewAttachment, hash: ContentHash) -> Self {
    Self {
      id: AttachmentId::generate(),
      name: new.name.clone(),
      media: new.media,
      size: new.size(),
      hash,
      dimensions: new.dimensions,
    }
  }

  /// Rebuilds a kept attachment.
  pub fn restore(
    id: AttachmentId,
    (name, media): (AttachmentName, MediaType),
    (size, hash): (u64, ContentHash),
    dimensions: Option<Dimensions>,
  ) -> Self {
    Self {
      id,
      name,
      media,
      size,
      hash,
      dimensions,
    }
  }

  pub const fn id(&self) -> AttachmentId {
    self.id
  }

  pub const fn name(&self) -> &AttachmentName {
    &self.name
  }

  pub const fn media(&self) -> MediaType {
    self.media
  }

  pub const fn size(&self) -> u64 {
    self.size
  }

  pub const fn hash(&self) -> &ContentHash {
    &self.hash
  }

  pub const fn dimensions(&self) -> Option<Dimensions> {
    self.dimensions
  }

  /// The name of the project's copy in its attachments folder: its id and its kind's extension.
  pub fn file_name(&self) -> String {
    format!("{}.{}", self.id, self.media.extension())
  }
}

/// The text a message of attachments only is kept with, since a message always says something: their names, one a
/// line. A conversation shows such a message by its attachments alone ([`is_names_only`]).
pub fn names_text<'a>(names: impl IntoIterator<Item = &'a AttachmentName>) -> String {
  names
    .into_iter()
    .map(AttachmentName::as_str)
    .collect::<Vec<_>>()
    .join("\n")
}

/// Whether `text`, a message with `attachments`, says nothing but their names ([`names_text`]).
pub fn is_names_only(text: &str, attachments: &[Attachment]) -> bool {
  !attachments.is_empty() && text.trim() == names_text(attachments.iter().map(Attachment::name))
}
