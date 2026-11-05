//! A decision model chosen as the project's reviewer (`docs/design/decision-reviewer.md`): its connection and key read
//! on the computer the review runs on. An explicit decision choice keeps its model even when it cannot be reached, so
//! its failure reaches the person.

use orchester_core::{
  id::DecisionConnectionId,
  run::review::{ReviewerChoice, default_decision_threshold},
};

use super::setup::{DecisionSetup, ReviewerSetup, ReviewerSources};

/// The decision reviewer `choice` names, connection `id`, as the computer of `sources` has it; unreachable when the
/// computer has no such connection or cannot read it.
pub(super) fn decision_setup(
  sources: &ReviewerSources,
  id: DecisionConnectionId,
  choice: &ReviewerChoice,
) -> ReviewerSetup {
  let unreachable = || ReviewerSetup::Unreachable {
    model: choice.model.clone(),
    connection: None,
  };
  let connection = match sources.computer.decision_connection(id) {
    Ok(Some(connection)) => connection,
    Ok(None) => return unreachable(),
    Err(error) => {
      tracing::warn!(%error, "cannot read the decision reviewer's connection");
      return unreachable();
    }
  };
  let key = connection.key().and_then(|id| match sources.computer.secret(id) {
    Ok(Some(secret)) if !secret.is_archived() => sources
      .computer
      .secret_value(sources.project, &secret)
      // `.ok()`: keychain diagnostics can carry private data; a missing key fails the review with a typed failure.
      .ok(),
    _ => None,
  });
  ReviewerSetup::Decision(DecisionSetup {
    connection,
    model: choice.model.clone(),
    threshold: choice.threshold.unwrap_or_else(default_decision_threshold),
    key,
  })
}
