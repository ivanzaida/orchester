//! Server-sent events as model APIs stream them (`docs/architecture/native-agent.md` §3.0): `event:` and
//! `data:` fields, several `data:` lines joined with a line break, an empty line ending an event, and
//! comment lines (`:`), which DeepSeek sends as keep-alives, ignored. Lines end in LF, CRLF or a lone
//! CR, and a byte order mark at the start is dropped, as the SSE standard says. Bytes arrive in chunks
//! that may split a line or a UTF-8 character anywhere, so lines are cut from bytes and decoded whole.
//! An event may hold at most [`MAX_EVENT`] bytes, so a stream that never ends a line or an event cannot
//! grow without bound while it keeps sending bytes.

use std::mem;

use thiserror::Error;

/// The most bytes one line, and one event's data, may hold.
pub const MAX_EVENT: usize = 16 * 1024 * 1024;

/// One event of the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
  /// The `event:` field; `None` when the event named none.
  pub name: Option<String>,
  pub data: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SseError {
  #[error("an event of the stream is longer than {MAX_EVENT} bytes")]
  TooLong,
}

/// Reads events from the chunks of a response body.
#[derive(Debug, Default)]
pub struct SseParser {
  /// The bytes of a line not yet ended.
  line: Vec<u8>,
  name: Option<String>,
  data: Option<String>,
  /// The last line ended in CR: an LF right after it belongs to that line end.
  after_cr: bool,
  /// A first line was read, so a byte order mark can no longer come.
  started: bool,
}

impl SseParser {
  pub fn new() -> Self {
    Self::default()
  }

  /// The events `chunk` completes.
  pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, SseError> {
    let mut events = Vec::new();
    let mut rest = chunk;
    while let Some(end) = rest.iter().position(|byte| matches!(byte, b'\n' | b'\r')) {
      let ending = rest[end];
      if self.after_cr && end == 0 && ending == b'\n' {
        self.after_cr = false;
        rest = &rest[1..];
        continue;
      }
      self.push(&rest[..end])?;
      rest = &rest[end + 1..];
      self.after_cr = ending == b'\r';
      let line = mem::take(&mut self.line);
      let text = String::from_utf8_lossy(&line);
      let text: &str = if mem::replace(&mut self.started, true) {
        &text
      } else {
        text.strip_prefix('\u{feff}').unwrap_or(&text)
      };
      if let Some(event) = self.line_ended(text)? {
        events.push(event);
      }
    }
    if !rest.is_empty() {
      self.after_cr = false;
    }
    self.push(rest)?;
    Ok(events)
  }

  fn push(&mut self, bytes: &[u8]) -> Result<(), SseError> {
    if self.line.len() + bytes.len() > MAX_EVENT {
      return Err(SseError::TooLong);
    }
    self.line.extend_from_slice(bytes);
    Ok(())
  }

  fn line_ended(&mut self, line: &str) -> Result<Option<SseEvent>, SseError> {
    if line.is_empty() {
      return Ok(self.dispatch());
    }
    if line.starts_with(':') {
      return Ok(None);
    }
    let (field, value) = line.split_once(':').unwrap_or((line, ""));
    let value = value.strip_prefix(' ').unwrap_or(value);
    match field {
      "event" => self.name = Some(value.to_owned()),
      "data" => match &mut self.data {
        Some(data) if data.len() + 1 + value.len() > MAX_EVENT => return Err(SseError::TooLong),
        Some(data) => {
          data.push('\n');
          data.push_str(value);
        }
        None => self.data = Some(value.to_owned()),
      },
      // `id:` and `retry:` matter for reconnecting browsers, not for one request's stream.
      _ => {}
    }
    Ok(None)
  }

  /// The event the lines since the last empty line make up; one without data is no event.
  fn dispatch(&mut self) -> Option<SseEvent> {
    let name = self.name.take();
    let data = self.data.take()?;
    Some(SseEvent { name, data })
  }
}
