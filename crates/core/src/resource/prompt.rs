use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::variable::Variable;

const OPEN: &str = "{{";
const CLOSE: &str = "}}";
const ESCAPE: char = '\\';

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptError {
  #[error("the placeholder opened at byte {offset} has no closing `}}}}`")]
  UnclosedPlaceholder { offset: usize },
  #[error("`{name}` at byte {offset} is not a variable in the catalog")]
  UnknownVariable { name: String, offset: usize },
}

/// Markdown prompt text whose `{{variable}}` placeholders all name catalog variables.
/// Placeholders are data references, never expressions; surrounding spaces inside the braces
/// are ignored. `\{{` is a literal `{{` (e.g. a template example in a prompt); rendering drops
/// the backslash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PromptText {
  text: String,
  variables: Vec<Variable>,
}

impl PromptText {
  pub fn new(text: &str) -> Result<Self, PromptError> {
    Self::try_from(text.to_owned())
  }

  pub fn as_str(&self) -> &str {
    &self.text
  }

  /// Referenced variables in order of first use, without repeats.
  pub fn variables(&self) -> &[Variable] {
    &self.variables
  }

  pub fn is_blank(&self) -> bool {
    self.text.trim().is_empty()
  }

  /// The text with every placeholder replaced by `value(variable)` and `\{{` escapes turned
  /// into `{{`. The first variable without a value is returned instead: a prompt never goes
  /// out with a placeholder silently left empty.
  pub fn render(&self, mut value: impl FnMut(Variable) -> Option<String>) -> Result<String, Variable> {
    let text = &self.text;
    let mut rendered = String::with_capacity(text.len());
    let mut start = 0;
    while let Some(open) = text[start..].find(OPEN) {
      let offset = start + open;
      let inner_start = offset + OPEN.len();
      if text[..offset].ends_with(ESCAPE) {
        rendered.push_str(&text[start..offset - ESCAPE.len_utf8()]);
        rendered.push_str(OPEN);
        start = inner_start;
        continue;
      }
      let inner_len = text[inner_start..]
        .find(CLOSE)
        .expect("construction checked that every placeholder is closed");
      let variable = text[inner_start..inner_start + inner_len]
        .trim()
        .parse::<Variable>()
        .expect("construction checked that every placeholder names a variable");
      rendered.push_str(&text[start..offset]);
      rendered.push_str(&value(variable).ok_or(variable)?);
      start = inner_start + inner_len + CLOSE.len();
    }
    rendered.push_str(&text[start..]);
    Ok(rendered)
  }
}

impl TryFrom<String> for PromptText {
  type Error = PromptError;

  fn try_from(text: String) -> Result<Self, PromptError> {
    Ok(Self {
      variables: placeholders(&text)?,
      text,
    })
  }
}

impl From<PromptText> for String {
  fn from(prompt: PromptText) -> Self {
    prompt.text
  }
}

fn placeholders(text: &str) -> Result<Vec<Variable>, PromptError> {
  let mut variables = Vec::new();
  let mut start = 0;
  while let Some(open) = text[start..].find(OPEN) {
    let offset = start + open;
    let inner_start = offset + OPEN.len();
    if text[..offset].ends_with(ESCAPE) {
      start = inner_start;
      continue;
    }
    let inner_len = text[inner_start..]
      .find(CLOSE)
      .ok_or(PromptError::UnclosedPlaceholder { offset })?;
    let name = text[inner_start..inner_start + inner_len].trim();
    let variable = name.parse::<Variable>().map_err(|_| PromptError::UnknownVariable {
      name: name.to_owned(),
      offset,
    })?;
    if !variables.contains(&variable) {
      variables.push(variable);
    }
    start = inner_start + inner_len + CLOSE.len();
  }
  Ok(variables)
}
