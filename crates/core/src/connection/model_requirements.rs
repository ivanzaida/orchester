//! The CLI version a model needs, as the provider's API said when it refused an older CLI. No
//! documented source lists them: Claude Code's changelog names models it added, not the versions
//! the API requires (checked 2026-09-30). The one source is the refusal itself, e.g. Opus 5.5 to
//! Claude Code 2.1.89: "Claude Code 2.1.89 does not support this model; version 2.1.280 or newer is
//! required" (`docs/evidence/claude-code-e2e-2026-09-30.md`). A run that gets it records the
//! version for its model on its connection ([`super::AgentConnection::record_model_requirement`]);
//! nothing is known before that, and no version is guessed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::compare_versions;
use crate::resource::ModelId;

/// The lowest CLI version each model needs, by model id, as refusals reported it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelRequirements(BTreeMap<String, String>);

impl ModelRequirements {
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// The version `model` needs, if a refusal said so.
  pub fn required(&self, model: &ModelId) -> Option<&str> {
    self.0.get(model.as_str()).map(String::as_str)
  }

  /// Records that `model` needs `version` or newer; a higher version already known stays.
  pub fn record(&mut self, model: &ModelId, version: &str) {
    let known = self.required(model);
    if compare_versions(Some(version), known).is_gt() {
      self.0.insert(model.as_str().to_owned(), version.to_owned());
    }
  }

  /// Every model with its version, by model id.
  pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
    self.0.iter().map(|(model, version)| (model.as_str(), version.as_str()))
  }
}

/// A refusal of the model to the installed CLI: the CLI's version and the one the model needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliTooOld {
  pub installed: String,
  pub required: String,
}

/// The refusal in `message`, an error the CLI reported for a turn:
/// `… Claude Code <installed> does not support this model; version <required> or newer is required …`.
pub fn parse_cli_too_old(message: &str) -> Option<CliTooOld> {
  const BEFORE: &str = "Claude Code ";
  const MIDDLE: &str = " does not support this model; version ";
  const AFTER: &str = " or newer is required";
  let (head, tail) = message.split_once(MIDDLE)?;
  let installed = head.rsplit_once(BEFORE)?.1;
  let required = tail.split_once(AFTER)?.0;
  (is_version(installed) && is_version(required)).then(|| CliTooOld {
    installed: installed.to_owned(),
    required: required.to_owned(),
  })
}

/// Digits separated by single dots, like `2.1.280`.
fn is_version(text: &str) -> bool {
  !text.is_empty()
    && text
      .split('.')
      .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}
