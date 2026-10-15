//! Additional Agents verification cases. No real CLI or runner is started.
use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CatalogModel, CatalogSource, CheckResult, EndpointPreset,
    ModelCatalog,
  },
  id::{ConnectionId, IdempotencyKey, SecretId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    StepPermissions,
  },
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, ResourceKind, Slug, Step, StepKey, StepKind,
  },
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ConnectionStore, ResourceStore, RunStore, TaskStore},
  task::TaskDetails,
};

use crate::kontur::{Kontur, SeedError, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let mut claude = None;
  for stored in kontur.db.connections()? {
    let Ok(mut connection) = AgentConnection::try_from(stored) else {
      continue;
    };
    let Some(mut check) = connection.last_check().cloned() else {
      continue;
    };
    if let CheckResult::Available { catalog, .. } = &mut check.result {
      *catalog = ModelCatalog::new(
        CatalogSource::Provider,
        connection
          .models()
          .iter()
          .map(|model| CatalogModel::new(model.id().clone(), vec!["low".into(), "medium".into(), "high".into()]))
          .collect(),
      );
    }
    connection.record_check(check)?;
    if connection.provider() == AgentProvider::ClaudeCodeCli {
      connection.record_model_requirement(&model("qa-unlisted-model"), "2.1.280");
      claude = Some(connection.id());
    }
    kontur.db.save_connection(&connection)?;
  }
  let claude = claude.ok_or_else(|| SeedError::Invalid("the Agents fixture needs Claude".into()))?;
  profile(
    kontur,
    "CLI warning",
    claude,
    "qa-unlisted-model",
    AgentPermissions::most_restrictive(
      AgentProvider::ClaudeCodeCli,
      orchester_core::connection::invocation::Platform::current(),
    ),
  )?;
  let commands = AllowedCommands::new(vec![CommandPrefix::new("cargo test").expect("fixture prefix")])
    .expect("fixture command list");
  profile(
    kontur,
    "Allowed commands",
    claude,
    "claude-sonnet-5-5",
    AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(commands),
      NetworkAccess::On,
    ),
  )?;
  profile(
    kontur,
    "Missing connection",
    ConnectionId::generate(),
    "qa-model",
    AgentPermissions::before_permissions(),
  )?;
  let blocked = AgentConnection::native(
    ConnectionId::generate(),
    name("OpenAI · unavailable"),
    AgentProvider::OpenAiApi,
    ApiEndpoint::new(
      EndpointPreset::Official,
      BaseUrl::new("https://api.openai.com").expect("fixture URL"),
      SecretId::generate(),
    ),
    kontur.now,
  )?;
  kontur.db.create_connection(&blocked)?;
  profile(
    kontur,
    "Cannot enforce",
    blocked.id(),
    "qa-model",
    AgentPermissions::before_permissions(),
  )?;
  active_override(kontur)
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("fixture model id")
}
fn profile(
  kontur: &mut Kontur,
  title: &str,
  connection: ConnectionId,
  model_id: &str,
  permissions: AgentPermissions,
) -> Result<(), SeedError> {
  let body = AgentProfile::new(
    name(title),
    "Agents verification fixture.".into(),
    PromptText::new("Use the profile’s recorded permissions.").expect("fixture prompt"),
    connection,
    model(model_id),
    ProviderSettings::default(),
  )
  .with_permissions(permissions);
  kontur.db.create_resource(
    Slug::from_name(title).expect("fixture slug"),
    name(title),
    &body,
    kontur.now,
  )?;
  Ok(())
}

fn active_override(kontur: &mut Kontur) -> Result<(), SeedError> {
  let resource = kontur
    .db
    .resources(ResourceKind::AgentProfile)?
    .into_iter()
    .find(|resource| resource.name().as_str() == "Builder")
    .ok_or_else(|| SeedError::Invalid("the Agents fixture needs Builder".into()))?;
  let profile = kontur
    .db
    .latest_version::<AgentProfile>(resource.id())?
    .ok_or_else(|| SeedError::Invalid("Builder must be published".into()))?;
  let key = StepKey::new("implement").expect("fixture key");
  let body = Pipeline::new(vec![Step {
    key: key.clone(),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(resource.id()),
      repository: RepositoryAccess::Write,
      model: Some(model("gpt-5.4-mini")),
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Implement the change.").expect("fixture prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("fixture pipeline");
  let pipeline = kontur.db.create_resource(
    Slug::from_name("Model override").expect("slug"),
    name("Model override"),
    &body,
    kontur.now,
  )?;
  let version = kontur.db.publish_draft::<Pipeline>(pipeline.id(), kontur.now)?;
  let snapshot = RunSnapshot::build(
    pipeline.id(),
    &version,
    PublishedVersions {
      profiles: &BTreeMap::from([(resource.id(), profile)]),
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )?;
  record_active(kontur, snapshot, &key)
}

fn record_active(kontur: &mut Kontur, snapshot: RunSnapshot, key: &StepKey) -> Result<(), SeedError> {
  let pipeline = snapshot.pipeline();
  let mut task = kontur
    .db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: name("Verify the pinned agent profile"),
        brief: "Recorded QA run; no process is started.".into(),
        acceptance_criteria: vec!["The active profile version is visible.".into()],
        pipeline: Some(pipeline.resource),
        limits: crate::kontur::limits(),
      }],
      kontur.now,
    )?
    .remove(0);
  task.queue(pipeline.number)?;
  let task = kontur.db.save_task(&task)?.saved().ok_or(SeedError::TaskChanged)?;
  let mut run = kontur.db.create_run_of_queued_task(
    IdempotencyKey::generate(),
    NewRun {
      task: task.id(),
      snapshot,
      branch: BranchName::for_next_run(task.key(), 0),
      base: kontur.base.clone(),
      base_branch: Some(BranchName::new("main").expect("branch")),
      budget: task.details().limits.budget(),
      max_duration: task.details().limits.max_duration(),
    },
    kontur.now,
    None,
  )?;
  let events = run.start(kontur.now)?;
  kontur.db.append_events(&run, &events)?;
  let execution = run.latest_execution(key).ok_or(SeedError::NotAtWork)?.id();
  let events = run.start_step(execution, kontur.now)?;
  kontur.db.append_events(&run, &events)?;
  Ok(())
}
