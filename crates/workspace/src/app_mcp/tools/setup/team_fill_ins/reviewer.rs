//! `set_reviewer_account`: what a team reviewer runs on here, as `set_profile_account` says it for a profile. A
//! language reviewer runs on one of the person's accounts of the CLI kind its version names, for which the project
//! gets its connection when it has none; a decision reviewer on a decision model's server, and sending requests to
//! a server that is not on this computer is a person's alone.

use orchester_core::{
  connection::DecisionConnection,
  resource::{ResourceKind, TeamReviewer},
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Spec,
    runs_on::{Accounts, Named, account_connection},
  },
  refusal, require_team_project, team_named,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  team_fill_ins::FillInError,
};

pub(super) const SPEC: Spec = Spec {
  name: "set_reviewer_account",
  description: "Chooses what a team reviewer runs on on this computer, as the reviewer's Runs on does in a team \
                project: `reviewer` is a team reviewer by id or name. A language reviewer takes `account`, a \
                sign-in or an API provider of the CLI kind its newest version names, by id or name \
                (list_accounts; `default` is the CLI's Default); the project gets its connection on the account \
                when it has none. A decision reviewer takes a decision model's server by id or name, one the \
                reviewer's destinations allow (a server on this computer always is). The choice is kept on this \
                computer only. Choosing a decision server that is not on this computer sends the project's \
                requests there, so it is refused as person_only. Refused as team_project_required in a project \
                that is not shared with a team, as reviewer_not_found for a reviewer that is not the team's, as \
                connection_does_not_fit for an account of another kind, as server_not_allowed for a decision \
                server the reviewer does not list, and as not_team_form for a reviewer whose newest version is \
                not in the team's form (publish it again). Returns the reviewer and what it runs on.",
  schema: schema_for_type::<SetReviewerAccount>,
  read_only: false,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetReviewerAccount {
  /// The project's id, or its name.
  pub project: String,
  /// The team reviewer's id, or its name.
  pub reviewer: String,
  /// What it runs on, by id or name: one of the person's accounts for a language reviewer, a decision model's
  /// server for a decision reviewer.
  pub account: String,
}

pub(super) async fn set_reviewer_account(services: &Services, args: SetReviewerAccount) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    require_team_project(&db)?;
    let reviewer = team_named(&services, &open.repo_root, ResourceKind::TeamReviewer, &args.reviewer)?
      .ok_or_else(|| ToolError::not_found(Missing::Reviewer, &args.reviewer))?;
    let kind = match reviewer.body::<TeamReviewer>() {
      None => None,
      Some(Ok(body)) => Some(body),
      Some(Err(_)) => return Err(refusal(FillInError::ReviewerNotTeamForm(reviewer.id))),
    };
    let (id, name) = if kind.as_ref().is_some_and(|body| body.cli().is_none()) {
      let connection = decision_named(&db, &args.account)?;
      services
        .workspace
        .pick_team_decision_connection(&open.repo_root, reviewer.id, connection.id(), Answerer::Automation)
        .map_err(refusal)?;
      (connection.id().to_string(), connection.name().as_str().to_owned())
    } else {
      let cli = kind.as_ref().and_then(TeamReviewer::cli);
      let (id, name) = match Accounts::load(&services)?.named(cli, &args.account)? {
        Named::SignIn(account) => (
          account.id().to_string(),
          account.name().map_or("default", |name| name.as_str()).to_owned(),
        ),
        Named::Api(account) => (account.id().to_string(), account.name().as_str().to_owned()),
      };
      // The project's connection on the account, made now when it has none.
      let connection = account_connection(&services, &open.repo_root, None, &id)?;
      services
        .workspace
        .pick_team_reviewer_connection(&open.repo_root, reviewer.id, connection)
        .map_err(refusal)?;
      (id, name)
    };
    Ok(json!({
      "reviewer": { "id": reviewer.id.to_string(), "name": reviewer.name.as_str() },
      "runs_on": { "id": id, "name": name },
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
