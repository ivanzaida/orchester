//! An HTTP tool's inputs and outputs as typed, one per line (A32 `gLLEv`, L20 `Kb6Vk`): `name → query|header|body KEY`
//! for what the agent gives, `name ← status`, `name ← header NAME` or `name ← body PATH` for what it gets back, the
//! arrows the app shows for MCP variables (`->` and `<-` are taken too). Each line is checked on its own, so the field
//! says every line that is wrong, by its number among all lines; empty lines are left out.

use std::collections::BTreeMap;

use orchester_core::resource::{FieldName, RequestTarget, ResponseSource};

use crate::messages::Message;

const INPUT_ARROWS: [&str; 2] = ["→", "->"];
const OUTPUT_ARROWS: [&str; 2] = ["←", "<-"];
const QUERY: &str = "query";
const HEADER: &str = "header";
const BODY: &str = "body";
const STATUS: &str = "status";

/// The inputs of `text`, or why each wrong line is wrong.
pub fn parse_inputs(text: &str) -> Result<BTreeMap<FieldName, RequestTarget>, Vec<Message>> {
  parse(text, &INPUT_ARROWS, input_target)
}

/// The outputs of `text`, or why each wrong line is wrong.
pub fn parse_outputs(text: &str) -> Result<BTreeMap<FieldName, ResponseSource>, Vec<Message>> {
  parse(text, &OUTPUT_ARROWS, output_source)
}

/// `inputs` one per line, as [`parse_inputs`] reads them.
pub fn inputs_text(inputs: &BTreeMap<FieldName, RequestTarget>) -> String {
  inputs
    .iter()
    .map(|(name, target)| {
      let (place, key) = match target {
        RequestTarget::Query(key) => (QUERY, key),
        RequestTarget::Header(key) => (HEADER, key),
        RequestTarget::Body(key) => (BODY, key),
      };
      format!("{name} {} {place} {key}", INPUT_ARROWS[0])
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// `outputs` one per line, as [`parse_outputs`] reads them.
pub fn outputs_text(outputs: &BTreeMap<FieldName, ResponseSource>) -> String {
  outputs
    .iter()
    .map(|(name, source)| match source {
      ResponseSource::Status => format!("{name} {} {STATUS}", OUTPUT_ARROWS[0]),
      ResponseSource::Header(key) => format!("{name} {} {HEADER} {key}", OUTPUT_ARROWS[0]),
      ResponseSource::Body(key) => format!("{name} {} {BODY} {key}", OUTPUT_ARROWS[0]),
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Where one line puts or reads its value: its place and the key after it, already split; `line` numbers its message.
type Place<T> = fn(line: usize, place: &str, key: &str) -> Result<T, Message>;

fn parse<T>(text: &str, arrows: &[&str], place: Place<T>) -> Result<BTreeMap<FieldName, T>, Vec<Message>> {
  let mut parsed = BTreeMap::new();
  let mut errors = Vec::new();
  let lines = text.lines().enumerate().filter(|(_, line)| !line.trim().is_empty());
  for (index, line) in lines {
    let number = index + 1;
    match parse_line(number, line, arrows, place) {
      Ok((name, _)) if parsed.contains_key(&name) => {
        errors.push(at(number, "tools.errors.line.twice").with("name", name.to_string()))
      }
      Ok((name, value)) => {
        parsed.insert(name, value);
      }
      Err(error) => errors.push(error),
    }
  }
  if errors.is_empty() { Ok(parsed) } else { Err(errors) }
}

fn parse_line<T>(number: usize, line: &str, arrows: &[&str], place: Place<T>) -> Result<(FieldName, T), Message> {
  let shape = || {
    at(
      number,
      if arrows == INPUT_ARROWS {
        "tools.errors.line.inputShape"
      } else {
        "tools.errors.line.outputShape"
      },
    )
  };
  let (name, rest) = arrows
    .iter()
    .find_map(|arrow| line.split_once(arrow))
    .ok_or_else(shape)?;
  let name = name.trim();
  let field = FieldName::new(name).map_err(|_| at(number, "tools.errors.line.name").with("name", name.to_owned()))?;
  let rest = rest.trim();
  let (kind, key) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
  if kind.is_empty() {
    return Err(shape());
  }
  Ok((field, place(number, kind, key.trim())?))
}

fn input_target(number: usize, place: &str, key: &str) -> Result<RequestTarget, Message> {
  if key.is_empty() && [QUERY, HEADER, BODY].contains(&place) {
    return Err(needs_key(number, place));
  }
  let target = match place {
    QUERY => RequestTarget::Query(key.to_owned()),
    HEADER => RequestTarget::Header(key.to_owned()),
    BODY => RequestTarget::Body(key.to_owned()),
    other => return Err(at(number, "tools.errors.line.inputPlace").with("place", other.to_owned())),
  };
  if !target.is_valid() {
    return Err(invalid_key(number, place, key));
  }
  if matches!(&target, RequestTarget::Header(name) if name.eq_ignore_ascii_case("authorization")) {
    return Err(at(number, "tools.errors.line.reserved"));
  }
  Ok(target)
}

fn output_source(number: usize, place: &str, key: &str) -> Result<ResponseSource, Message> {
  let source = match place {
    STATUS if key.is_empty() => return Ok(ResponseSource::Status),
    STATUS => return Err(at(number, "tools.errors.line.statusKey")),
    HEADER | BODY if key.is_empty() => return Err(needs_key(number, place)),
    HEADER => ResponseSource::Header(key.to_owned()),
    BODY => ResponseSource::Body(key.to_owned()),
    other => return Err(at(number, "tools.errors.line.outputPlace").with("place", other.to_owned())),
  };
  if source.is_valid() {
    Ok(source)
  } else {
    Err(invalid_key(number, place, key))
  }
}

/// A body takes a path; a query or a header a name.
fn needs_key(number: usize, place: &str) -> Message {
  let key = if place == BODY {
    "tools.errors.line.needsPath"
  } else {
    "tools.errors.line.needsName"
  };
  at(number, key).with("place", place.to_owned())
}

fn invalid_key(number: usize, place: &str, key: &str) -> Message {
  let message = if place == BODY {
    "tools.errors.line.invalidPath"
  } else {
    "tools.errors.line.invalidName"
  };
  at(number, message)
    .with("place", place.to_owned())
    .with("key", key.to_owned())
}

/// The message `key`, one of `tools.errors.line.*`, about line `number`.
fn at(number: usize, key: &'static str) -> Message {
  Message::new(key).with("line", number.to_string())
}
