//! Supported private fixtures for the ORC-3 verifier and the step states of the redesigned Pipelines (`03b` `g6Ecy`);
//! no agents or commands are started. A pipeline is stored only as published versions (D-032), so every pipeline here
//! is created as its first version and each of its steps runs something, even one that cannot run here.
use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ResourceId, SecretId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, StepCommands, StepNetwork,
    StepPermissions,
  },
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, DatabaseQuery, DeliverStep, DeniedBehavior, HttpAuth,
    HttpMethod, HttpRequest, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug,
    StepKey, StepKind, Tool, ToolAction,
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
  kontur.catalogue()?;
  problems(kontur)?;
  narrowed(kontur)?;
  variants(kontur)
}

/// Creates pipeline `title` as its first version `body`.
pub(super) fn create(kontur: &mut Kontur, title: &str, body: &Pipeline) -> Result<ResourceId, SeedError> {
  let (resource, _) = kontur.db.create_published(
    Slug::from_name(title).expect("fixture title yields a slug"),
    name(title),
    body,
    kontur.now,
  )?;
  Ok(resource.id())
}

/// The agent step that runs `profile` and only reads.
fn runs(profile: ResourceId) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    model: None,
    repository: RepositoryAccess::ReadOnly,
    permissions: StepPermissions::default(),
  }
}

/// *Publication problems*, whose cards stand in amber: a step whose profile is no longer available, one whose profile
/// is not published, and one whose tool cannot run as a step.
fn problems(kontur: &mut Kontur) -> Result<(), SeedError> {
  let unpublished = profile(kontur, "Unpublished reviewer", &["cargo test"], false)?;
  let tool = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("fixture query")),
    Some(policy()),
  )
  .expect("database tool carries an access policy");
  let (tool, _) = kontur.db.create_published(
    Slug::new("unsupported-database").expect("fixture slug"),
    name("Unsupported database"),
    &tool,
    kontur.now,
  )?;
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let first = edit.add_step(name("Missing profile"), None);
  edit.update(&first, |s| s.kind = runs(ResourceId::generate()));
  let second = edit.add_step(name("Unpublished profile"), Some(&first));
  edit.update(&second, |s| s.kind = runs(unpublished));
  let third = edit.add_step(name("Unsupported tool"), Some(&second));
  edit.update(&third, |s| s.kind = StepKind::Tool { tool: Some(tool.id()) });
  create(
    kontur,
    "Publication problems",
    &edit.validate().expect("steps that run what is not here are valid"),
  )?;
  Ok(())
}
pub(super) fn profile(
  kontur: &mut Kontur,
  title: &str,
  commands: &[&str],
  publish: bool,
) -> Result<ResourceId, SeedError> {
  let allowed = AllowedCommands::new(
    commands
      .iter()
      .map(|command| CommandPrefix::new(command).expect("fixture prefix"))
      .collect(),
  )
  .expect("unique fixture prefixes");
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(allowed),
    NetworkAccess::On,
  );
  agent(kontur, (title, AgentProvider::ClaudeCodeCli), permissions, publish)
}

/// A profile called `title` on a connection of its own to `provider`, with `permissions`; published when `publish`.
fn agent(
  kontur: &mut Kontur,
  (title, provider): (&str, AgentProvider),
  permissions: AgentPermissions,
  publish: bool,
) -> Result<ResourceId, SeedError> {
  let connection = ConnectionId::generate();
  let program = match provider {
    AgentProvider::CodexCli => ".orchester/unused-qa-codex",
    _ => ".orchester/unused-qa-claude",
  };
  kontur.db.create_connection(&Connection::new(
    connection,
    name(title),
    ConnectionKind::Agent(provider),
    AgentEndpoint::Program(kontur.repo.join(program)),
    kontur.now,
  )?)?;
  let body = AgentProfile::new(
    name(title),
    String::new(),
    PromptText::new("").expect("empty prompt"),
    connection,
    ModelId::new("qa-model").expect("fixture model"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions);
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
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
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
  create(
    kontur,
    "Narrowed permissions",
    &edit.validate().expect("valid narrowed step"),
  )?;
  Ok(())
}

/// Two pipelines for 03b's (`g6Ecy`) variants that need data of their own. In `Status checks`, *Check status page* runs
/// an HTTP tool, which cannot run as a step yet. In `Read-only review`, *Codex review* and *Summarize* run on a Codex
/// CLI profile with any command, so a step can narrow neither its commands nor, on Windows, its network; its Deliver
/// step follows steps that only read the repository, so no step can resolve conflicts and no command tool step runs
/// Check again; and making *Codex review* run after *Summarize* draws a cycle.
fn variants(kontur: &mut Kontur) -> Result<(), SeedError> {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://status.kontur.dev/api/summary".into(),
    "status.kontur.dev".into(),
    HttpAuth::None,
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("fixture request");
  let tool = Tool::new("", ToolAction::Http(request), Some(policy())).expect("HTTP tool carries an access policy");
  let tool_id = kontur
    .db
    .create_resource(
      Slug::new("status-page").expect("fixture slug"),
      name("Status page"),
      &tool,
      kontur.now,
    )?
    .id();
  kontur.db.publish_draft::<Tool>(tool_id, kontur.now)?;
  let codex = agent(
    kontur,
    ("Codex reviewer", AgentProvider::CodexCli),
    AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    ),
    true,
  )?;
  // On its own: a tool that cannot run keeps a save of it from publishing, which would cloud the others' states.
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let status = edit.add_step(name("Check status page"), None);
  edit.update(&status, |s| s.kind = StepKind::Tool { tool: Some(tool_id) });
  create(
    kontur,
    "Status checks",
    &edit.validate().expect("a step that runs a tool is valid"),
  )?;
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let review = edit.add_step(name("Codex review"), None);
  let summary = edit.add_step(name("Summarize"), Some(&review));
  for key in [&review, &summary] {
    edit.update(key, |s| {
      s.kind = StepKind::Agent {
        profile: Some(codex),
        model: None,
        repository: RepositoryAccess::ReadOnly,
        permissions: StepPermissions::default(),
      }
    });
  }
  edit.set_deliver(Some(DeliverStep::default()));
  create(
    kontur,
    "Read-only review",
    &edit.validate().expect("read-only steps and a Deliver step"),
  )?;
  Ok(())
}

/// An access policy that lets no caller in, which HTTP and database tools must carry.
fn policy() -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::new(),
    name("QA"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("empty caller list denies everyone")
}

/// The steps drawn as the flow they run in (`Steps` `uSoll` in `Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): Feature
/// delivery with its gate, its checks and review side by side, their failure routes and its Deliver step; *Publication
/// problems*, whose cards stand in amber (a step whose profile is no longer available, one whose profile is not
/// published, one whose tool cannot run); *Gated group*, whose two steps in a group both wait for the person; and
/// *Independent chains*, whose steps wait for only part of what runs before them, drawn by stage with what each waits
/// for (P16 `T9KJFP`).
pub fn flow(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.delivering_catalogue()?;
  problems(kontur)?;
  let builder = profile(kontur, "Flow builder", &["cargo test"], true)?;
  gated_group(kontur, builder)?;
  chains(kontur, builder)?;
  Ok(())
}

/// *Prepare*, then *Check* and *Review* side by side, each ending in a gate; every step runs `builder`.
fn gated_group(kontur: &mut Kontur, builder: ResourceId) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let prepare = edit.add_step(name("Prepare"), None);
  let check = edit.add_step(name("Check"), Some(&prepare));
  let review = edit.add_step(name("Review"), Some(&prepare));
  for key in [&prepare, &check, &review] {
    edit.update(key, |step| step.kind = runs(builder));
  }
  for key in [&check, &review] {
    edit.update(key, |step| step.approval_gate = true);
  }
  create(kontur, "Gated group", &edit.validate().expect("gated steps are valid"))?;
  Ok(())
}

/// *Fetch* then *Parse*, and apart from them *Lint* then *Report*: *Parse* waits for *Fetch* only and *Report* for
/// *Lint* only, each starting before the other chain's step ends. Every step runs `builder`.
fn chains(kontur: &mut Kontur, builder: ResourceId) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let fetch = edit.add_step(name("Fetch"), None);
  let lint = edit.add_step(name("Lint"), None);
  let parse = edit.add_step(name("Parse"), None);
  let report = edit.add_step(name("Report"), None);
  edit.set_dependency(&parse, &fetch, true);
  edit.set_dependency(&report, &lint, true);
  for key in [&fetch, &lint, &parse, &report] {
    edit.update(key, |step| step.kind = runs(builder));
  }
  create(
    kontur,
    "Independent chains",
    &edit.validate().expect("two chains are valid steps"),
  )?;
  Ok(())
}

/// The design's catalogue with Feature delivery's next version, whose *Create pull request* (06) waits for *Run
/// checks* (04) only, not for *Review* (05) beside it (`Pipelines · v2` P16 `T9KJFP`; open step 06 for its *Waits
/// for*, P16b `P1853`).
pub fn branched(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let id = catalogue.feature_delivery.id();
  let latest = kontur
    .db
    .latest_version::<Pipeline>(id)?
    .ok_or_else(|| SeedError::Invalid("Feature delivery has no version".to_owned()))?;
  let mut edit = PipelineEdit::new(latest.body().clone());
  let checks = StepKey::new("run-checks").expect("a starter step's key");
  edit.update(
    &StepKey::new("create-pull-request").expect("a starter step's key"),
    |step| step.depends_on = std::iter::once(checks).collect(),
  );
  let body = edit.validate().map_err(|error| SeedError::Invalid(error.to_string()))?;
  kontur
    .db
    .publish_pipeline_edit(id, &body, latest.number(), None, kontur.now)?;
  Ok(())
}
