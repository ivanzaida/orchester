//! Exact display ranges of values substituted into a preview source.
use std::ops::Range;

use orchester_core::resource::{PromptText, Variable, prompt_references};
/// Escaped examples remain ordinary text; only actual substitutions are marked.
pub fn rendered_ranges(
  prompt: &PromptText,
  mut value: impl FnMut(Variable) -> Option<String>,
) -> (String, Vec<Range<usize>>) {
  let raw = prompt.as_str();
  let mut text = String::new();
  let mut ranges = Vec::new();
  let mut start = 0;
  for reference in prompt_references(raw) {
    let reference = reference.expect("PromptText validated every reference");
    text.push_str(&raw[start..reference.range.start].replace("\\{{", "{{"));
    if let Some(value) = value(reference.variable) {
      let from = text.len();
      text.push_str(&value);
      if !value.is_empty() {
        ranges.push(from..text.len());
      }
    } else {
      text.push_str(&raw[reference.range.clone()]);
    }
    start = reference.range.end;
  }
  text.push_str(&raw[start..].replace("\\{{", "{{"));
  (text, ranges)
}
