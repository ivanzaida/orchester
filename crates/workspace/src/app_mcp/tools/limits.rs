//! `update_project_limits`: Project settings › Runs › Limits through Orchester's MCP server, saved with the
//! Limits tab's checks and through its service, so a changed agent count reaches the project's
//! runner at once. `list_projects` shows each project's limits as [`limits_json`] writes them, with the computer's
//! *Services at once*, which the same tool changes: an application setting that every project's runner reads.
//! Limits answer no tool request in a person's place, so, like the code index, changing them needs
//! no permission beyond the server's token, except raising or removing the daily budget: that lets runs the day's
//! budget stopped go on, a person's decision ([`DecisionKind::Limit`]) automation takes only with their consent.

use std::path::Path;

use orchester_core::{
  limits::{ComputerServiceSlots, ProjectLimits},
  store::SettingsStore,
  tokens::{TokenBudget, parse_budget},
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
  budget::{
    LimitsErrors, ReadLimits, agent_slots, budget_error, computer_service_slots, qa_slots, service_slots, time_limit,
    whole_minutes,
  },
  decisions::{DecisionKind, not_allowed},
  messages::Message,
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
  /// Tokens all runs of the project may use per day, with no maximum; `null` for no limit.
  /// Unchanged when left out. Raising it or `null` is refused as automation_not_allowed (nothing
  /// is saved) unless a person turned on Let automation answer approvals; lowering it is not.
  #[serde(default, deserialize_with = "wire::limit")]
  pub daily_budget: Option<Option<u64>>,
  /// Whole minutes of active agent time prefilled in New task, above 0; `null` for no default time
  /// limit. Unchanged when left out.
  #[serde(default, deserialize_with = "wire::limit")]
  pub default_time_limit_minutes: Option<Option<u64>>,
  /// How many agents work at once across the project's runs, 1 to 16. Applies at once: more let
  /// queued work start, fewer stop no agent and hold new work. Unchanged when left out.
  pub agent_slots: Option<u64>,
  /// How many QA apps (`start_qa_app`) the project's runs may have open at once, 1 to 16; a start
  /// beyond it waits until one closes. Applies from the next start: fewer close no open app.
  /// Unchanged when left out.
  pub qa_slots: Option<u64>,
  /// The project's share of the computer's Services at once: how many services (`start_service`, a stack's each, a
  /// test suite's own process) the project's runs may run at once, 1 to the computer's limit; a start beyond it waits
  /// until enough stop. Applies from the next start: fewer stop no service. Unchanged when left out.
  pub service_slots: Option<u64>,
  /// The computer's Services at once, for every project, 1 to 32: how many services all projects' runs together may
  /// run at once. A project's share above it counts as it. Applies from the next start. Unchanged when left out.
  pub computer_service_slots: Option<u64>,
}

pub async fn update_project_limits(services: &Services, args: UpdateProjectLimits) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let may_raise = DecisionKind::Limit.read_consent(&services.workspace)?;
    let computer = match args.computer_service_slots {
      Some(count) => computer_service_slots(count),
      None => Ok(services.workspace.service_slots()?),
    };
    // Read and written in one transaction: a person's save in the window cannot slip in between.
    // A refusal saves nothing; why is kept for the result.
    let mut refusal = None;
    update_limits(&services.runners, &root, |stored| {
      match changed(stored, &args, &computer) {
        Err(errors) => {
          refusal = Some(refused(*errors));
          None
        }
        // Lowering it, or leaving it as it is, takes no consent.
        Ok(limits) if stored.raises_daily_budget(limits) && !may_raise => {
          refusal = Some(not_allowed());
          None
        }
        Ok(limits) => Some(limits),
      }
    })?;
    if let Some(refusal) = refusal {
      return Err(refusal);
    }
    // Checked with the project's limits above, so a refusal saved neither.
    if let (Some(_), Ok(computer)) = (args.computer_service_slots, computer) {
      services.workspace.set_service_slots(computer)?;
    }
    services.changes.record(open.project.id(), Outside::Limits);
    let limits = limits_json(limits_in(&root)?, services.workspace.service_slots()?);
    Ok(json!({ "limits": limits }))
  })
  .await
}

/// `stored` with the limits `args` gives, checked as the Limits tab checks its fields; the project's share of
/// *Services at once* against the computer's limit `computer`, the one given or the one kept.
fn changed(
  stored: ProjectLimits,
  args: &UpdateProjectLimits,
  computer: &Result<ComputerServiceSlots, Message>,
) -> Result<ProjectLimits, Box<LimitsErrors>> {
  // Read as a field's digits: zero and more than a budget holds are refused in the tab's words.
  let budget = |tokens: u64| parse_budget(&tokens.to_string(), "").map_err(|error| budget_error(error, ""));
  let default_time_limit = match args.default_time_limit_minutes {
    Some(Some(minutes)) => time_limit(minutes).map(Some),
    Some(None) => Ok(None),
    None => Ok(stored.default_time_limit()),
  };
  let limit = computer.as_ref().ok().copied();
  let read = ReadLimits {
    default_run_budget: match args.default_run_budget {
      Some(given) => given.map(budget).transpose(),
      None => Ok(stored.default_run_budget()),
    },
    daily_budget: match args.daily_budget {
      Some(Some(given)) => budget(given),
      Some(None) | None => Ok(stored.daily_budget()),
    },
    default_time_limit,
    agent_slots: args.agent_slots.map_or(Ok(stored.agent_slots()), agent_slots),
    qa_slots: args.qa_slots.map_or(Ok(stored.qa_slots()), qa_slots),
    service_slots: args
      .service_slots
      .map_or(Ok(stored.service_slots()), |count| service_slots(count, limit)),
  }
  .limits();
  let read = match (read, computer) {
    (read, Ok(_)) => read,
    (Ok(_), Err(why)) => Err(Box::new(LimitsErrors {
      computer_service_slots: Some(why.clone()),
      ..LimitsErrors::default()
    })),
    (Err(mut errors), Err(why)) => {
      errors.computer_service_slots = Some(why.clone());
      Err(errors)
    }
  };
  read.map(|limits| {
    limits.with_daily_budget_limit(match args.daily_budget {
      Some(None) => None,
      Some(Some(_)) => Some(limits.daily_budget()),
      None => stored.daily_budget_limit(),
    })
  })
}

/// Refused with the reason of each limit that cannot be saved, in the words of the Limits tab.
fn refused(errors: LimitsErrors) -> ToolError {
  let LimitsErrors {
    default_run_budget,
    daily_budget,
    default_time_limit,
    agent_slots,
    qa_slots,
    service_slots,
    computer_service_slots,
  } = errors;
  let reasons = [
    default_run_budget,
    daily_budget,
    default_time_limit,
    agent_slots,
    qa_slots,
    service_slots,
    computer_service_slots,
  ]
  .into_iter()
  .flatten()
  .collect();
  ToolError::refused("invalid_limits", reasons)
}

/// A project's limits as the tools give them, with the computer's *Services at once* `computer`; `null` is no limit.
pub fn limits_json(limits: ProjectLimits, computer: ComputerServiceSlots) -> Value {
  json!({
    "default_run_budget": limits.default_run_budget().map(TokenBudget::get),
    "daily_budget": limits.daily_budget_limit().map(TokenBudget::get),
    "default_time_limit_minutes": limits.default_time_limit().map(whole_minutes),
    "agent_slots": limits.agent_slots().get().get(),
    "qa_slots": limits.qa_slots().get().get(),
    "service_slots": limits.service_slots().get().get(),
    "computer_service_slots": computer.get().get(),
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
