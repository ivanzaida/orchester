//! `pick_reviewer_connection`: the connection a team reviewer runs on here, as `pick_profile_connection` does for a
//! profile. A language reviewer runs on an agent connection of the CLI kind its version names; a decision reviewer on a
//! decision connection, and sending requests to a server that is not on this computer is a person's alone.

use orchester_core::{
  connection::DecisionConnection,
  resource::{ResourceKind, TeamReviewer},
  run::Answerer,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Spec, connections::agent_named},
  refusal, require_team_project, team_resource_named,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  team_fill_ins::FillInError,
};

pub(super) const SPEC: Spec = Spec {
  name: "pick_reviewer_connection",
  description: "Chooses the connection a team reviewer runs on on this computer, as the reviewer's Connection field \
                does in a team project: `reviewer` is a team reviewer by id or name. A language reviewer takes an \
                agent connection of the project (by id or name) of the CLI kind its newest version names; a decision \
                reviewer takes a decision connection of the project whose server the reviewer's destinations allow \
                (a server on this computer always is). The pick is kept on this computer only. Choosing a decision \
                connection whose server is not on this computer sends the project's requests there, so it is \
                refused as person_only. Refused as team_project_required in a project that is not shared with a \
                team, as reviewer_not_found for a reviewer that is not the team's, as connection_does_not_fit for a \
                connection of the other kind, as server_not_allowed for a decision server the reviewer does not \
                list, and as not_team_form for a reviewer whose newest version is not in the team's form (publish \
                it again). Returns the pick.",
  schema: schema_for_type::<PickReviewerConnection>,
  read_only: false,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct PickReviewerConnection {
  /// The project's id, or its name.
  pub project: String,
  /// The team reviewer's id, or its name.
  pub reviewer: String,
  /// The connection's id, or its name: an agent connection for a language reviewer, a decision connection for a
  /// decision reviewer.
  pub connection: String,
}

pub(super) async fn pick_reviewer_connection(
  services: &Services,
  args: PickReviewerConnection,
) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    require_team_project(&db)?;
    let reviewer = team_resource_named(&db, ResourceKind::TeamReviewer, &args.reviewer)?
      .ok_or_else(|| ToolError::not_found(Missing::Reviewer, &args.reviewer))?;
    let kind = match db.latest_version::<TeamReviewer>(reviewer.id()) {
      Ok(version) => version,
      Err(StoreError::Corrupt { .. }) => return Err(refusal(FillInError::ReviewerNotTeamForm(reviewer.id()))),
      Err(other) => return Err(other.into()),
    };
    let (id, name) = if kind.is_some_and(|version| version.body().cli().is_none()) {
      let connection = decision_named(&db, &args.connection)?;
      services
        .workspace
        .pick_team_decision_connection(&open.repo_root, reviewer.id(), connection.id(), Answerer::Automation)
        .map_err(refusal)?;
      (connection.id().to_string(), connection.name().as_str().to_owned())
    } else {
      let connection = agent_named(&db, &args.connection)?;
      services
        .workspace
        .pick_team_reviewer_connection(&open.repo_root, reviewer.id(), connection.id())
        .map_err(refusal)?;
      (connection.id().to_string(), connection.name().as_str().to_owned())
    };
    Ok(json!({
      "reviewer": { "id": reviewer.id().to_string(), "name": reviewer.name().as_str() },
      "connection": { "id": id, "name": name },
    }))
  })
  .await
}

fn decision_named(db: &ProjectDb, named: &str) -> Result<DecisionConnection, ToolError> {
  db.decision_connections()?
    .into_iter()
    .find(|connection| connection.id().to_string() == named || connection.name().as_str().eq_ignore_ascii_case(named))
    .ok_or_else(|| ToolError::not_found(Missing::Connection, named))
}
