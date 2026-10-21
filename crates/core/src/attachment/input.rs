//! How an agent is given a message's attachments, each after a line that names it (its name, kind and size), in the
//! order the person attached them:
//!
//! - Claude Code: one stream-json user message whose content holds the message's text, then for each file its line and
//!   an `image` or `document` block of its content in base64, or for text a text block of the line and the text.
//! - Codex App Server: text input items as for Claude Code, and an image as a `localImage` item naming the project's
//!   copy, which App Server reads itself.
//!
//! A file the connection does not take ([`super::support`]) is only named, with why it was not given: the send was
//! checked before, so this happens only when the connection changed since. The contents are read when the input is
//! made and go nowhere else.

use std::{fs, io, path::PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Attachment, AttachmentKind, MediaType, support};
use crate::connection::AgentProvider;

/// A kept attachment and the project's copy of it, as an agent is given it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachedFile {
  pub attachment: Attachment,
  pub path: PathBuf,
}

impl AttachedFile {
  /// The content of the project's copy.
  pub fn read(&self) -> io::Result<Vec<u8>> {
    fs::read(&self.path)
  }

  /// The content of a text file's copy.
  pub fn read_text(&self) -> io::Result<String> {
    String::from_utf8(self.read()?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8 text"))
  }

  /// The content in base64, as the APIs take an image or a document.
  pub fn base64(&self) -> io::Result<String> {
    Ok(STANDARD.encode(self.read()?))
  }
}

/// The line that names `attachment` to the agent, before its content.
pub fn header(attachment: &Attachment) -> String {
  format!(
    "[Attachment: {} ({}, {} bytes)]",
    attachment.name().as_str(),
    attachment.media().as_str(),
    attachment.size()
  )
}

/// The line that tells the agent `attachment` was not given to it, as `provider` does not take its kind.
pub fn not_given(attachment: &Attachment, provider: AgentProvider) -> String {
  let kind = match attachment.media().kind() {
    AttachmentKind::Image => "images",
    AttachmentKind::Pdf => "PDF files",
    AttachmentKind::Text => "text files",
  };
  format!(
    "{} This attachment was not given to you: your connection ({provider:?}) does not take {kind}.",
    header(attachment)
  )
}

/// The content blocks of a Claude Code user message: `text`, then each of `files`.
pub fn claude_content(text: &str, files: &[AttachedFile]) -> io::Result<Vec<Value>> {
  let mut content = vec![text_block(text)];
  for file in files {
    let line = header(&file.attachment);
    match file.attachment.media() {
      MediaType::Text => content.push(text_block(&format!("{line}\n{}", file.read_text()?))),
      MediaType::Pdf => {
        content.push(text_block(&line));
        content.push(json!({"type": "document", "source": {
          "type": "base64", "media_type": MediaType::Pdf.as_str(), "data": file.base64()?
        }}));
      }
      media => {
        content.push(text_block(&line));
        content.push(json!({"type": "image", "source": {
          "type": "base64", "media_type": media.as_str(), "data": file.base64()?
        }}));
      }
    }
  }
  Ok(content)
}

/// `text` and `files` as a stream-json user message, one line of Claude Code's input.
pub fn claude_line(text: &str, files: &[AttachedFile]) -> io::Result<String> {
  Ok(user_line(&claude_content(text, files)?))
}

/// The content blocks `content` as a stream-json user message, one line of Claude Code's input.
pub fn user_line(content: &[Value]) -> String {
  json!({"type": "user", "message": {"role": "user", "content": content}}).to_string()
}

/// The input items of a Codex turn or steer: `text`, then each of `files`; a PDF, which Codex does not take, only
/// named.
pub fn codex_items(text: &str, files: &[AttachedFile]) -> io::Result<Vec<Value>> {
  let mut items = vec![text_block(text)];
  for file in files {
    let attachment = &file.attachment;
    match attachment.media().kind() {
      AttachmentKind::Text => items.push(text_block(&format!("{}\n{}", header(attachment), file.read_text()?))),
      kind if !support::takes(AgentProvider::CodexCli, kind) => {
        items.push(text_block(&not_given(attachment, AgentProvider::CodexCli)));
      }
      _ => {
        // App Server reads the image itself: a copy that is gone fails here, as the other kinds do.
        fs::metadata(&file.path)?;
        items.push(text_block(&header(attachment)));
        items.push(json!({"type": "localImage", "path": file.path}));
      }
    }
  }
  Ok(items)
}

/// The text of `items`, joined a line apart as Claude Code prints a message back and Codex shows a user message it
/// took: what tells which message the agent took.
pub fn taken_text(items: &[Value]) -> String {
  items
    .iter()
    .filter_map(|item| item.get("text").and_then(Value::as_str))
    .collect::<Vec<_>>()
    .join("\n")
}

fn text_block(text: &str) -> Value {
  json!({"type": "text", "text": text})
}
