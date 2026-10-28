//! Project settings shared by the window and automation. Blocking work belongs off the UI thread.

mod base_branch;
mod repository;
use std::path::Path;

pub use base_branch::{
  BaseBranchChoice, BaseBranchRead, BaseBranchSaved, base_branch_in, landing, save_base_branch_in,
};
use orchester_core::{
  agent_files::RepositoryAgentFiles,
  code_index::CodeIndexSetting,
  connection::{AgentConnection, Connection},
  limits::ProjectLimits,
  resource::ModelId,
  run::{
    ProjectRule, ProjectRuleId,
    review::{AutoModeSettings, ReviewerChoice, default_review_model, resolve_reviewer, reviewer_kind},
  },
  store::{ConnectionStore, SettingsStore},
  time::Timestamp,
  tokens::{TokenBudget, grouped},
};
use orchester_store::{ProjectDb, StoreError};
pub use repository::{RepositoryRead, check_repository_in, repository_in};

use crate::{
  agents::form::{effort_options, listed_models},
  budget::{
    LimitsErrors, ReadLimits, minutes_text, read_agent_slots, read_budget, read_minutes, read_optional_budget,
    read_qa_slots,
  },
  messages::{self, Message},
  runner::Runners,
  tasks::{run_approvals::unavailable_key, usage::update_limits},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
  pub limits: ProjectLimits,
  pub agent_files: RepositoryAgentFiles,
  pub code_index: CodeIndexSetting,
  pub auto_mode: AutoModeSettings,
  pub auto_runs: bool,
  pub connections: Vec<Connection>,
  /// The project's active approval rules, oldest first (*Approve for every run of this project*).
  pub approval_rules: Vec<ProjectRule>,
  /// *Tell working agents when the base branch moves* (`crate::base_moved`); on until a person turns it off.
  pub base_notices: bool,
}

pub fn load_in(root: &Path) -> Result<Settings, StoreError> {
  let db = ProjectDb::open(root)?;
  Ok(Settings {
    limits: db.project_limits()?,
    agent_files: db.repository_agent_files()?,
    code_index: db.code_index()?,
    auto_mode: db.auto_mode_settings()?,
    auto_runs: db.has_auto_mode_runs()?,
    connections: db.connections()?,
    approval_rules: db
      .project_approval_rules()?
      .into_iter()
      .filter(ProjectRule::is_active)
      .collect(),
    base_notices: db.tells_base_moves()?,
  })
}

/// Turns *Tell working agents when the base branch moves* on or off. While it is off, moves queue no notice and the
/// runner delivers none of those still pending.
pub fn save_base_notices_in(root: &Path, on: bool) -> Result<(), StoreError> {
  ProjectDb::open(root)?.save_tells_base_moves(on)
}

/// Revokes the project's approval rule `rule`: the requests it answered ask again from the next one on. A rule
/// revoked already stays as it is.
pub fn revoke_project_rule_in(root: &Path, rule: ProjectRuleId) -> Result<(), StoreError> {
  ProjectDb::open(root)?
    .revoke_project_approval_rule(rule, Timestamp::now())
    .map(|_| ())
}

/// Fields of the new page; all three limits may be absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LimitsText {
  pub default: String,
  pub daily: String,
  pub minutes: String,
  pub slots: String,
  /// *QA apps at once*.
  pub qa: String,
}
impl LimitsText {
  pub fn of(limits: ProjectLimits, separator: &str) -> Self {
    let budget = |value: Option<TokenBudget>| value.map(|v| grouped(v.get(), separator)).unwrap_or_default();
    Self {
      default: budget(limits.default_run_budget()),
      daily: budget(limits.daily_budget_limit()),
      minutes: minutes_text(limits.default_time_limit()),
      slots: limits.agent_slots().get().to_string(),
      qa: limits.qa_slots().get().to_string(),
    }
  }

  pub fn limits(&self, base: ProjectLimits, separator: &str) -> Result<ProjectLimits, Box<LimitsErrors>> {
    let daily = read_optional_budget(&self.daily, separator);
    let finite = match &daily {
      Ok(value) => value.unwrap_or(base.daily_budget()).get().to_string(),
      Err(_) => self.daily.clone(),
    };
    let mut result = ReadLimits {
      default_run_budget: read_optional_budget(&self.default, separator),
      daily_budget: read_budget(&finite, separator),
      default_time_limit: read_minutes(&self.minutes),
      agent_slots: read_agent_slots(&self.slots),
      qa_slots: read_qa_slots(&self.qa),
    }
    .limits();
    if let Err(error) = daily {
      match &mut result {
        Err(errors) => errors.daily_budget = Some(error),
        Ok(_) => {
          return Err(Box::new(LimitsErrors {
            daily_budget: Some(error),
            ..LimitsErrors::default()
          }));
        }
      }
    }
    result.map(|limits| {
      limits.with_daily_budget_limit(read_optional_budget(&self.daily, separator).expect("daily was validated"))
    })
  }
}

/// Only changed fields are rebased onto the stored limits, in the store's write transaction.
pub fn save_limits_in(
  root: &Path,
  runners: &Runners,
  base: ProjectLimits,
  asked: ProjectLimits,
) -> Result<ProjectLimits, StoreError> {
  update_limits(runners, root, |stored| {
    let limits = ProjectLimits::new(
      pick(
        base.default_run_budget(),
        asked.default_run_budget(),
        stored.default_run_budget(),
      ),
      pick(base.daily_budget(), asked.daily_budget(), stored.daily_budget()),
      pick(
        base.default_time_limit(),
        asked.default_time_limit(),
        stored.default_time_limit(),
      ),
    )
    .expect("rebasing fields of validated ProjectLimits preserves their invariants");
    Some(
      limits
        .with_daily_budget_limit(pick(
          base.daily_budget_limit(),
          asked.daily_budget_limit(),
          stored.daily_budget_limit(),
        ))
        .with_agent_slots(pick(base.agent_slots(), asked.agent_slots(), stored.agent_slots()))
        .with_qa_slots(pick(base.qa_slots(), asked.qa_slots(), stored.qa_slots())),
    )
  })?;
  ProjectDb::open(root)?.project_limits()
}

pub fn save_agent_files_in(root: &Path, enabled: bool) -> Result<(), StoreError> {
  ProjectDb::open(root)?.save_repository_agent_files(if enabled {
    RepositoryAgentFiles::On
  } else {
    RepositoryAgentFiles::Off
  })
}

/// The reviewer entered by the person, checked with the same model and effort rules as automation.
pub fn reviewer(
  settings: &Settings,
  connection: &str,
  model: &str,
  effort: &str,
) -> Result<Option<ReviewerChoice>, Message> {
  let selected = if connection.is_empty() {
    if (model.is_empty() || model == default_model(settings, "")) && effort.is_empty() {
      return Ok(None);
    }
    // Editing the default's model or effort pins the connection that default resolves to.
    resolve_reviewer(&AutoModeSettings::default(), &settings.connections)
      .map_err(|why| Message::new(unavailable_key(why)))?
      .connection
  } else {
    settings
      .connections
      .iter()
      .find(|c| c.id().to_string() == connection)
      .ok_or_else(|| {
        Message::new(unavailable_key(
          orchester_core::run::review::ReviewerUnavailable::ConnectionGone,
        ))
      })?
  };
  if reviewer_kind(selected).is_none() {
    return Err(Message::new(unavailable_key(
      orchester_core::run::review::ReviewerUnavailable::Unsupported,
    )));
  }
  let model = ModelId::new(model).map_err(|_| Message::new("errors.modelInvalid"))?;
  let agent = AgentConnection::try_from(selected.clone()).map_err(|_| Message::new("errors.connectionRequired"))?;
  if !effort.is_empty() && !effort_options(&agent, &model).contains(&effort.to_owned()) {
    return Err(Message::new("errors.modelInvalid"));
  }
  Ok(Some(ReviewerChoice {
    connection: selected.id(),
    model,
    effort: (!effort.is_empty()).then(|| effort.to_owned()),
  }))
}

/// Auto mode answers for the person; enabling it or changing its active reviewer needs confirmation.
pub fn auto_widens(base: &AutoModeSettings, asked: &AutoModeSettings, auto_runs: bool) -> bool {
  (asked.new_runs && !base.new_runs) || ((asked.new_runs || auto_runs) && base.reviewer != asked.reviewer)
}

pub fn save_auto_mode_in(
  root: &Path,
  base: &AutoModeSettings,
  asked: &AutoModeSettings,
  confirmed: bool,
) -> Result<(), Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  if auto_widens(
    base,
    asked,
    db.has_auto_mode_runs().map_err(|e| messages::store_error(&e))?,
  ) && !confirmed
  {
    return Err(Message::new("projectSettings.personOnly"));
  }
  let connections = db.connections().map_err(|error| messages::store_error(&error))?;
  if asked.new_runs
    || (db.has_auto_mode_runs().map_err(|e| messages::store_error(&e))? && base.reviewer != asked.reviewer)
  {
    resolve_reviewer(asked, &connections).map_err(|why| Message::new(unavailable_key(why)))?;
  }
  use orchester_store::AutoModeSave;
  match db
    .save_auto_mode_if_current(base, asked, |auto_runs| {
      confirmed || !auto_widens(base, asked, auto_runs)
    })
    .map_err(|error| messages::store_error(&error))?
  {
    AutoModeSave::Saved => Ok(()),
    AutoModeSave::ChangedOutside => Err(Message::new("projectSettings.changedOutside")),
    AutoModeSave::Refused => Err(Message::new("projectSettings.personOnly")),
  }
}

fn pick<T: Copy + PartialEq>(before: T, entered: T, stored: T) -> T {
  if before != entered { entered } else { stored }
}

/// Options and read-back of the reviewer form, resolved by the domain rather than the component.
pub struct ReviewOptions {
  pub connections: Vec<(String, String)>,
  /// Agent CLI connections on an API provider, by id and name: listed, unavailable, since the reviewer starts its CLI
  /// on its own sign-in (`docs/design/api-providers.md`).
  pub on_providers: Vec<(String, String)>,
  pub default_connection: Option<String>,
  pub models: Vec<String>,
  pub efforts: Vec<String>,
  pub resolved: Result<crate::tasks::run_approvals::ReviewerShown, Message>,
}
pub fn review_options(settings: &Settings, connection: &str, model: &str) -> ReviewOptions {
  let resolved = resolve_reviewer(&settings.auto_mode, &settings.connections)
    .map(|r| crate::tasks::run_approvals::ReviewerShown {
      connection: r.connection.name().as_str().into(),
      model: r.model,
      effort: r.effort,
    })
    .map_err(|why| Message::new(unavailable_key(why)));
  let selected = settings
    .connections
    .iter()
    .find(|c| c.id().to_string() == connection)
    .or_else(|| {
      // A missing reviewer has no selected option; its refusal is retained in `resolved` above.
      resolve_reviewer(&settings.auto_mode, &settings.connections)
        .ok()
        .map(|r| r.connection)
    });
  let mut models = Vec::new();
  let mut efforts = Vec::new();
  // Non-agent connections have no model or effort options; reviewer validation refuses them above.
  if let Some(agent) = selected.and_then(|c| AgentConnection::try_from(c.clone()).ok()) {
    models = listed_models(&agent).iter().map(|m| m.as_str().to_owned()).collect();
    if let Ok(id) = ModelId::new(model) {
      efforts = effort_options(&agent, &id);
      if !models.contains(&model.to_owned()) {
        models.push(model.to_owned());
      }
    }
  }
  ReviewOptions {
    // No default caption when unavailable; the form retains the explicit refusal in resolved.
    default_connection: resolve_reviewer(&AutoModeSettings::default(), &settings.connections)
      .ok()
      .map(|r| r.connection.name().as_str().to_owned()),
    connections: settings
      .connections
      .iter()
      .filter(|c| reviewer_kind(c).is_some())
      .map(|c| (c.id().to_string(), c.name().as_str().to_owned()))
      .collect(),
    on_providers: settings
      .connections
      .iter()
      .filter(|c| c.api_provider().is_some())
      .map(|c| (c.id().to_string(), c.name().as_str().to_owned()))
      .collect(),
    models,
    efforts,
    resolved,
  }
}

pub fn default_model(settings: &Settings, connection: &str) -> String {
  if connection.is_empty() {
    return resolve_reviewer(&AutoModeSettings::default(), &settings.connections)
      .map(|reviewer| reviewer.model.as_str().to_owned())
      .unwrap_or_default();
  }
  settings
    .connections
    .iter()
    .find(|c| c.id().to_string() == connection)
    .and_then(default_review_model)
    .map(|model| model.as_str().to_owned())
    .unwrap_or_default()
}
