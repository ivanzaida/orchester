//! What a person's change in the Auto mode section makes of the stored settings: another reviewer
//! connection with its default model, another model, another effort. A connection that cannot review
//! (gone, of a kind no reviewer runs on yet, or naming no model) is refused with the reason, and the
//! settings stay as they are.

use orchester_core::{
  connection::{AgentConnection, Connection},
  id::ConnectionId,
  resource::ModelId,
  run::review::{
    AutoModeSettings, ReviewerChoice, ReviewerUnavailable, default_review_model, resolve_reviewer, reviewer_kind,
  },
};

use crate::{agents::form::effort_options, messages::Message, tasks::run_approvals::unavailable_key};

/// The settings as stored, and the project's agent connections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Stored {
  pub(super) settings: AutoModeSettings,
  pub(super) connections: Vec<Connection>,
}

impl lurq::app::component::DevtoolsInspectable for Stored {}

impl Stored {
  /// The reviewer these settings resolve to: its connection, model and effort.
  pub(super) fn resolved(&self) -> Option<(&Connection, ModelId, Option<String>)> {
    resolve_reviewer(&self.settings, &self.connections)
      .ok()
      .map(|resolved| (resolved.connection, resolved.model, resolved.effort))
  }
}

/// A change of the settings: `None` when it changes nothing, an error when it is refused.
pub(super) type Change = Result<Option<AutoModeSettings>, Message>;

/// Why `connection` cannot review; `None` when it can.
pub(super) fn cannot_review(connection: &Connection) -> Option<ReviewerUnavailable> {
  if reviewer_kind(connection).is_none() {
    return Some(ReviewerUnavailable::Unsupported);
  }
  default_review_model(connection)
    .is_none()
    .then_some(ReviewerUnavailable::NoModel)
}

/// The settings with the reviewer on connection `pick` (`None`: the default) and its default model.
pub(super) fn with_connection(current: &Stored, pick: Option<ConnectionId>) -> Change {
  if pick == current.settings.reviewer.as_ref().map(|choice| choice.connection) {
    return Ok(None);
  }
  let Some(id) = pick else {
    return Ok(Some(AutoModeSettings {
      reviewer: None,
      ..current.settings.clone()
    }));
  };
  let refused = |why| Message::new(unavailable_key(why));
  let connection = current
    .connections
    .iter()
    .find(|connection| connection.id() == id)
    .ok_or_else(|| refused(ReviewerUnavailable::ConnectionGone))?;
  if let Some(why) = cannot_review(connection) {
    return Err(refused(why));
  }
  let model = default_review_model(connection).ok_or_else(|| refused(ReviewerUnavailable::NoModel))?;
  Ok(Some(AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: id,
      model,
      effort: None,
    }),
    ..current.settings.clone()
  }))
}

/// The settings with the resolved reviewer's connection reviewing with `model`; its effort stays when the
/// model takes it.
pub(super) fn with_model(current: &Stored, model: ModelId) -> Change {
  let Some((connection, resolved, effort)) = current.resolved() else {
    return Ok(None);
  };
  if resolved == model {
    return Ok(None);
  }
  let takes = |effort: &String| {
    AgentConnection::try_from(connection.clone()).is_ok_and(|agent| effort_options(&agent, &model).contains(effort))
  };
  Ok(Some(AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: connection.id(),
      effort: effort.filter(takes),
      model,
    }),
    ..current.settings.clone()
  }))
}

/// The settings with the resolved reviewer at `effort`.
pub(super) fn with_effort(current: &Stored, effort: Option<String>) -> Change {
  let Some((connection, model, resolved)) = current.resolved() else {
    return Ok(None);
  };
  Ok((resolved != effort).then(|| AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: connection.id(),
      model,
      effort,
    }),
    ..current.settings.clone()
  }))
}
