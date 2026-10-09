//! `update_project_limits`: Project settings › Limits through Orchester's MCP server, saved with the
//! Limits tab's checks and through its service, so a changed agent count reaches the project's
//! runner at once. `list_projects` shows each project's limits as [`limits_json`] writes them.
//! Limits answer no tool request in a person's place, so, like the code index, changing them needs
//! no permission beyond the server's token.

use std::path::Path;

use orchester_core::{
  limits::ProjectLimits,
  store::SettingsStore,
  tokens::{BudgetInputError, TokenBudget},
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  budget::{LimitsErrors, ReadLimits, agent_slots, budget_error, time_limit, whole_minutes},
  tasks::{
    outside::Outside,
    usage::{limits_in, update_limits},
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct UpdateProjectLimits {
  /// The project's id, or its name.
  pub project: String,
  /// Tokens prefilled in New task, with no maximum; `null` for no default budget. Unchanged when
  /// left out.
  #[serde(default, deserialize_with = "wire::limit")]
  pub default_run_budget: Option<Option<u64>>,
  /// Tokens all runs of the project may use per day. Unchanged when left out.
  pub daily_budget: Option<u64>,
  /// Whole minutes of active agent time prefilled in New task, above 0; `null` for no default time
  /// limit. Unchanged when left out.
  #[serde(default, deserialize_with = "wire::limit")]
  pub default_time_limit_minutes: Option<Option<u64>>,
  /// How many agents work at once across the project's runs, 1 to 16. Applies at once: more let
  /// queued work start, fewer stop no agent and hold new work. Unchanged when left out.
  pub agent_slots: Option<u64>,
}

pub async fn update_project_limits(services: &Services, args: UpdateProjectLimits) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    // Read and written in one transaction: a person's save in the window cannot slip in between.
    // A refusal saves nothing; why is kept for the result.
    let mut refusal = None;
    update_limits(&services.runners, &root, |stored| {
      changed(stored, &args).map_err(|errors| refusal = Some(errors)).ok()
    })?;
    if let Some(errors) = refusal {
      return Err(refused(*errors));
    }
    services.changes.record(open.project.id(), Outside::Limits);
    Ok(json!({ "limits": limits_json(limits_in(&root)?) }))
  })
  .await
}

/// `stored` with the limits `args` gives, checked as the Limits tab checks its fields.
fn changed(stored: ProjectLimits, args: &UpdateProjectLimits) -> Result<ProjectLimits, Box<LimitsErrors>> {
  let budget = |tokens: u64| TokenBudget::new(tokens).map_err(|_| budget_error(BudgetInputError::Zero, ""));
  let default_time_limit = match args.default_time_limit_minutes {
    Some(Some(minutes)) => time_limit(minutes).map(Some),
    Some(None) => Ok(None),
    None => Ok(stored.default_time_limit()),
  };
  ReadLimits {
    default_run_budget: match args.default_run_budget {
      Some(given) => given.map(budget).transpose(),
      None => Ok(stored.default_run_budget()),
    },
    daily_budget: args.daily_budget.map_or(Ok(stored.daily_budget()), budget),
    default_time_limit,
    agent_slots: args.agent_slots.map_or(Ok(stored.agent_slots()), agent_slots),
  }
  .limits()
}

/// Refused with the reason of each limit that cannot be saved, in the words of the Limits tab.
fn refused(errors: LimitsErrors) -> ToolError {
  let LimitsErrors {
    default_run_budget,
    daily_budget,
    default_time_limit,
    agent_slots,
  } = errors;
  let reasons = [default_run_budget, daily_budget, default_time_limit, agent_slots]
    .into_iter()
    .flatten()
    .collect();
  ToolError::refused("invalid_limits", reasons)
}

/// A project's limits as the tools give them; `null` is no limit.
pub fn limits_json(limits: ProjectLimits) -> Value {
  json!({
    "default_run_budget": limits.default_run_budget().map(TokenBudget::get),
    "daily_budget": limits.daily_budget().get(),
    "default_time_limit_minutes": limits.default_time_limit().map(whole_minutes),
    "agent_slots": limits.agent_slots().get().get(),
  })
}

/// The limits of the project at `root`, read without changing its database; `None` when they cannot
/// be read before the project is opened (its data waits for a migration) or reading failed, which
/// the log says.
pub fn limits_of(root: &Path) -> Option<ProjectLimits> {
  match ProjectDb::open_read_only(root).and_then(|db| db.project_limits()) {
    Ok(limits) => Some(limits),
    Err(StoreError::SchemaTooOld { .. }) => None,
    Err(error) => {
      tracing::warn!(%error, project = %root.display(), "cannot read a project's limits");
      None
    }
  }
}
