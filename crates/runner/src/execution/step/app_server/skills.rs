//! The skill the Orchestrator chat's turn loads as Codex's own (ORC-663, `orchester_core::connection::invocation::
//! CodexChatSkill`): once Codex says who is signed in, the session registers the skill's folder for this App Server
//! (`skills/extraRoots/set`), which Codex keeps in memory only, then reads the configuration as for any session. A step
//! carries no skill and goes straight to the configuration.

use orchester_core::connection::{app_server::Update, invocation::EXTRA_ROOTS_METHOD};
use serde_json::{Value, json};

use super::{AppServerFailure, AppServerSession, Phase, io_message, send};
use crate::supervisor::Supervised;

/// The id of the request that registers the skill the turn loads.
pub(super) const REQUEST: &str = "skill-roots";

impl AppServerSession {
  /// Codex said who is signed in: registers the turn's skill, if it carries one, or reads the configuration.
  pub(super) async fn signed_in(&mut self, process: &Supervised) -> Result<(), AppServerFailure> {
    let Some(skill) = &self.setup.skill else {
      return self.read_configuration(process).await;
    };
    self.phase = Phase::Skills;
    let params = skill.skill.extra_roots();
    send(
      process,
      json!({"id":REQUEST,"method":EXTRA_ROOTS_METHOD,"params":params}),
    )
    .await
    .map_err(|error| io_message(&error).into())
  }

  /// Codex answered the registration of the turn's skill (`value`): the turn loads it as Codex's own. A Codex that
  /// refuses it (one without the request) leaves the message as kept, the skill's text in its block, and the turn goes
  /// on; then the configuration is read.
  pub(super) async fn skill_registered(
    &mut self,
    value: &Value,
    process: &Supervised,
  ) -> Result<Vec<Update>, AppServerFailure> {
    if let Some(error) = value.get("error") {
      let reason = self.reason(error);
      tracing::warn!(%reason, "Codex did not register the chosen skill: it goes as text");
      self.setup.skill = None;
    }
    self.read_configuration(process).await?;
    Ok(Vec::new())
  }

  /// Asks Codex for the configuration it reads in the worktree, with its layers.
  async fn read_configuration(&mut self, process: &Supervised) -> Result<(), AppServerFailure> {
    self.phase = Phase::Configuration;
    let cwd = self.setup.worktree.to_string_lossy();
    send(
      process,
      json!({"id":6,"method":"config/read","params":{"cwd":cwd,"includeLayers":true}}),
    )
    .await
    .map_err(|error| io_message(&error).into())
  }
}
