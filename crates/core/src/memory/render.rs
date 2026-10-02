//! How a prompt shows a record. Every line of a record's body is quoted with `> `, so nothing a
//! record says, whoever wrote it, can start a heading or pass for a section of the prompt such as
//! "# When you finish".

use super::{InjectionReason, Memory, content::is_line_break};

/// `body` with every line quoted; an empty line becomes a bare `>`.
pub fn quoted(body: &str) -> String {
  body
    .replace("\r\n", "\n")
    .split(is_line_break)
    .map(|line| {
      if line.is_empty() {
        ">".to_owned()
      } else {
        format!("> {line}")
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// The prompt's text for `memory` chosen for `reason`, showing `body`: a handoff as its quoted body
/// under its group's heading, any other record as its kind and key on a line of their own, then its
/// quoted body.
pub(super) fn entry_text(memory: &Memory, reason: InjectionReason, body: &str) -> String {
  if reason == InjectionReason::Handoff {
    return quoted(body);
  }
  let content = memory.content();
  let key = content
    .key()
    // Quoted and escaped, so a key reads as data, never as part of the label.
    .map(|key| format!(" (key {:?})", key.as_str()))
    .unwrap_or_default();
  format!("{}{key}:\n{}", content.kind().label(), quoted(body))
}

/// `body` without the list items of section `heading` (a paragraph that starts with `heading:`)
/// that are in `items`; a section left without items goes as a whole.
pub(super) fn without_items(body: &str, heading: &str, items: &[&str]) -> String {
  let start = format!("{heading}:");
  body
    .split("\n\n")
    .filter_map(|section| {
      let mut lines = section.lines();
      if lines.next() != Some(start.as_str()) {
        return Some(section.to_owned());
      }
      let kept: Vec<&str> = lines
        .filter(|line| !line.strip_prefix("- ").is_some_and(|item| items.contains(&item.trim())))
        .collect();
      (!kept.is_empty()).then(|| format!("{start}\n{}", kept.join("\n")))
    })
    .collect::<Vec<_>>()
    .join("\n\n")
}
