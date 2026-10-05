//! Server-sent events as model APIs stream them (`docs/architecture/native-agent.md` §3.0): `event:` and
//! `data:` fields, several `data:` lines joined with a line break, an empty line ending an event, and
//! comment lines (`:`), which DeepSeek sends as keep-alives, ignored. Bytes arrive in chunks that may
//! split a line or a UTF-8 character anywhere, so lines are cut from bytes and decoded whole.

use std::mem;

/// One event of the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
  /// The `event:` field; `None` when the event named none.
  pub name: Option<String>,
  pub data: String,
}

/// Reads events from the chunks of a response body.
#[derive(Debug, Default)]
pub struct SseParser {
  /// The bytes of a line not yet ended.
  line: Vec<u8>,
  name: Option<String>,
  data: Option<String>,
}

impl SseParser {
  pub fn new() -> Self {
    Self::default()
  }

  /// The events `chunk` completes.
  pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut rest = chunk;
    while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
      self.line.extend_from_slice(&rest[..end]);
      rest = &rest[end + 1..];
      let mut line = mem::take(&mut self.line);
      if line.last() == Some(&b'\r') {
        line.pop();
      }
      if let Some(event) = self.line_ended(&String::from_utf8_lossy(&line)) {
        events.push(event);
      }
    }
    self.line.extend_from_slice(rest);
    events
  }

  fn line_ended(&mut self, line: &str) -> Option<SseEvent> {
    if line.is_empty() {
      return self.dispatch();
    }
    if line.starts_with(':') {
      return None;
    }
    let (field, value) = line.split_once(':').unwrap_or((line, ""));
    let value = value.strip_prefix(' ').unwrap_or(value);
    match field {
      "event" => self.name = Some(value.to_owned()),
      "data" => match &mut self.data {
        Some(data) => {
          data.push('\n');
          data.push_str(value);
        }
        None => self.data = Some(value.to_owned()),
      },
      // `id:` and `retry:` matter for reconnecting browsers, not for one request's stream.
      _ => {}
    }
    None
  }

  /// The event the lines since the last empty line make up; one without data is no event.
  fn dispatch(&mut self) -> Option<SseEvent> {
    let name = self.name.take();
    let data = self.data.take()?;
    Some(SseEvent { name, data })
  }
}
