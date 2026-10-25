//! The text of a conversation message.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::MessageError;

/// The text of a message: never blank, without the whitespace around it.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageBody(String);

impl MessageBody {
  pub fn new(text: &str) -> Result<Self, MessageError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(MessageError::Blank);
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for MessageBody {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.0, f)
  }
}

impl TryFrom<String> for MessageBody {
  type Error = MessageError;

  fn try_from(text: String) -> Result<Self, Self::Error> {
    Self::new(&text)
  }
}

impl From<MessageBody> for String {
  fn from(body: MessageBody) -> Self {
    body.0
  }
}
