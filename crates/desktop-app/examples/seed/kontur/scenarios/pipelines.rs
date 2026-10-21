//! Supported private fixtures for the ORC-3 verifier; no agents or commands are started.
use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId, SecretId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepCommands, StepNetwork,
    StepPermissions,
  },
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, DatabaseQuery, DeniedBehavior, Draft, ModelId, Pipeline,
    PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKind, Tool, ToolAction,
  },
  store::{ConnectionStore, ResourceStore},
};

use crate::kontur::{Kontur, SeedError, name};

pub fn empty(kontur: &mut Kontur) -> Result<(), SeedError> {
  // A new project with deliberately no catalogue: sI5K0's empty pipeline library.
  assert!(
    kontur
      .db
      .resources(orchester_core::resource::ResourceKind::Pipeline)?
      .is_empty()
  );
  Ok(())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let id = catalogue.feature_delivery.id();
  let latest = kontur
    .db
    .latest_version::<Pipeline>(id)?
    .expect("the catalogue publishes Feature delivery");
  kontur.db.save_draft(
    id,
    &Draft::new(
      latest
        .body()
        .clone()
        .with_description("Important saved unpublished work".into()),
      Some(latest.number()),
      kontur.now,
    ),
  )?;
  create(
    kontur,
    "Empty draft",
    &Pipeline::new(vec![]).expect("empty drafts are valid"),
  )?;
  problems(kontur)?;
  narrowed(kontur)
}

fn create(kontur: &mut Kontur, title: &str, body: &Pipeline) -> Result<ResourceId, SeedError> {
  Ok(
    kontur
      .db
      .create_resource(
        Slug::from_name(title).expect("fixture title yields a slug"),
        name(title),
        body,
        kontur.now,
      )?
      .id(),
  )
}
fn problems(kontur: &mut Kontur) -> Result<(), SeedError> {
  let unpublished = profile(kontur, "Unpublished reviewer", &["cargo test"], false)?;
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    name("QA"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("empty caller list denies everyone");
  let tool = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("fixture query")),
    Some(policy),
  )
  .expect("database tool carries an access policy");
  let tool_id = kontur
    .db
    .create_resource(
      Slug::new("unsupported-database").expect("fixture slug"),
      name("Unsupported database"),
      &tool,
      kontur.now,
    )?
    .id();
  kontur.db.publish_draft::<Tool>(tool_id, kontur.now)?;
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let first = edit.add_step(name("Missing profile"), None);
  let second = edit.add_step(name("Unpublished profile"), Some(&first));
  edit.update(&second, |s| {
    s.kind = StepKind::Agent {
      profile: Some(unpublished),
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions::default(),
    }
  });
  let third = edit.add_step(name("Unsupported tool"), Some(&second));
  edit.update(&third, |s| s.kind = StepKind::Tool { tool: Some(tool_id) });
  create(
    kontur,
    "Publication problems",
    &edit.validate().expect("incomplete executors are valid in drafts"),
  )?;
  Ok(())
}
fn profile(kontur: &mut Kontur, title: &str, commands: &[&str], publish: bool) -> Result<ResourceId, SeedError> {
  let connection = ConnectionId::generate();
  kontur.db.create_connection(&Connection::new(
    connection,
    name(title),
    ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
    AgentEndpoint::Program(kontur.repo.join(".orchester/unused-qa-claude")),
    kontur.now,
  )?)?;
  let allowed = AllowedCommands::new(
    commands
      .iter()
      .map(|command| CommandPrefix::new(command).expect("fixture prefix"))
      .collect(),
  )
  .expect("unique fixture prefixes");
  let body = AgentProfile::new(
    name(title),
    String::new(),
    PromptText::new("").expect("empty prompt"),
    connection,
    ModelId::new("qa-model").expect("fixture model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(allowed),
    NetworkAccess::On,
  ));
  let id = kontur
    .db
    .create_resource(
      Slug::from_name(title).expect("fixture slug"),
      name(title),
      &body,
      kontur.now,
    )?
    .id();
  if publish {
    kontur.db.publish_draft::<AgentProfile>(id, kontur.now)?;
  }
  Ok(id)
}
fn narrowed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let profile_id = profile(
    kontur,
    "Narrowable reviewer",
    &["cargo check", "cargo test", "git diff"],
    true,
  )?;
  profile(kontur, "Alternate reviewer", &["cargo test"], true)?;
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let key = edit.add_step(name("Review"), None);
  edit.update(&key, |s| {
    s.approval_gate = true;
    s.kind = StepKind::Agent {
      profile: Some(profile_id),
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions {
        commands: StepCommands::Only(
          AllowedCommands::new(vec![CommandPrefix::new("cargo check").expect("fixture command")]).expect("one command"),
        ),
        network: StepNetwork::AsProfile,
        interactive_tool_approvals: true,
        ..StepPermissions::default()
      },
    };
  });
  let id = create(
    kontur,
    "Narrowed permissions",
    &edit.validate().expect("valid narrowed step"),
  )?;
  kontur.db.publish_draft::<Pipeline>(id, kontur.now)?;
  Ok(())
}
