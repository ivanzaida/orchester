//! Editor ranges share the prompt grammar; decoration never changes validation or persisted bytes.
use orchester_core::resource::{PromptReference, prompt_references};

/// Known inline references while editing, including around invalid/partial expressions.
/// Form validation reports scanner errors; those expressions remain ordinary editable source.
/// A valid multiline expression remains valid prompt text but cannot have an inline pill.
pub fn inline_references(raw: &str) -> Vec<PromptReference> {
  prompt_references(raw)
    .filter_map(|reference| match reference {
      Ok(reference) if !raw[reference.range.clone()].contains(['\r', '\n']) => Some(reference),
      // Invalid expressions are already exposed by the form, not decorated as valid variables.
      Ok(_) | Err(_) => None,
    })
    .collect()
}
/// Replace actual grammar references only; escaped examples and unrelated text stay untouched.
pub fn replace_variable(
  raw: &str,
  from: orchester_core::resource::Variable,
  to: orchester_core::resource::Variable,
) -> String {
  let mut text = raw.to_owned();
  let references = prompt_references(raw)
    .filter_map(Result::ok)
    .filter(|r| r.variable == from)
    .collect::<Vec<_>>();
  for reference in references.into_iter().rev() {
    text.replace_range(reference.range, &format!("{{{{{}}}}}", to.name()));
  }
  text
}
