//! What a step in the hub's panel says around its *Agent* field (`Pipelines · v2` P2b `EGEos`, P2d `D4MDQO`; PV2-18):
//! *Open Builder* under the label for every profile, which opens the profile in the Agents hub and asks first when the
//! panel holds changes; a tool step's rule; a tool that cannot run here says why in place of *Instructions* and keeps
//! only the approval in *Gate and access*; and *When the step fails* read-only under *Run*.
//! `cargo test -p orchester-app --test agents pipeline_step_builder`.

use std::collections::BTreeMap;

use orchester_app::routes::{HubFilter, HubView, ProjectPage, Route};
use orchester_core::{
  id::{ResourceId, SecretId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, CommandAction, CommandProgram, DatabaseQuery,
    DeniedBehavior, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, Slug, StepKey,
    StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{
  harness::Agents,
  pipeline_hub::setup,
  pipeline_step::{keys, open_step, route},
  support::project_id,
};

const LINK: &str = "pipeline-hub-step-open-profile";
const DISCARD: &str = "pipeline-hub-leave-discard";
const KEEP: &str = "pipeline-hub-leave-cancel";
const HINT: &str = "team-hub-actions-hint";

fn name(text: &str) -> Name {
  Name::new(text).expect("name")
}

fn profile_page(id: ResourceId) -> Route {
  Route::Project(
    project_id(),
    ProjectPage::AgentsHub(HubView::new(HubFilter::Profiles, Some(id))),
  )
}

/// A database tool with access rules, which a step cannot run in this version, published.
fn database_tool(db: &mut ProjectDb) -> ResourceId {
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    name("QA"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("policy");
  let tool = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("query")),
    Some(policy),
  )
  .expect("tool");
  let id = db
    .create_resource(
      Slug::new("checks-db").expect("slug"),
      name("Checks"),
      &tool,
      Timestamp::now(),
    )
    .expect("the tool")
    .id();
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  id
}

/// A command tool without access rules, which a step runs, published.
fn command_tool(db: &mut ProjectDb) -> ResourceId {
  let action = ToolAction::Command(CommandAction::new(
    CommandProgram::new("cargo").expect("program"),
    Vec::new(),
  ));
  let tool = Tool::new("", action, None).expect("tool");
  let id = db
    .create_resource(Slug::new("lint").expect("slug"), name("Lint"), &tool, Timestamp::now())
    .expect("the tool")
    .id();
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
  id
}

/// A profile with a draft and no published version.
fn draft_profile(agents: &Agents, db: &mut ProjectDb) -> ResourceId {
  let connection = agents.listed().connections[0].id();
  let profile = AgentProfile::new(
    name("Reviewer"),
    "".into(),
    PromptText::new("").expect("instructions"),
    connection,
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  );
  db.create_resource(
    Slug::new("reviewer").expect("slug"),
    name("Reviewer"),
    &profile,
    Timestamp::now(),
  )
  .expect("the profile")
  .id()
}

/// A published pipeline whose step *Review* runs a profile with only a draft, whose step *Checks* runs the database
/// tool and whose step *Lint* runs the command tool.
fn problems(agents: &Agents) -> (ResourceId, ResourceId, [StepKey; 3]) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let tool = database_tool(&mut db);
  let lint_tool = command_tool(&mut db);
  let reviewer = draft_profile(agents, &mut db);
  let mut edit = PipelineEdit::new(Pipeline::new(Vec::new()).expect("a pipeline"));
  let review = edit.add_step(name("Review"), None);
  edit.update(&review, |step| {
    step.kind = StepKind::Agent {
      profile: Some(reviewer),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    };
  });
  let checks = edit.add_step(name("Checks"), Some(&review));
  edit.update(&checks, |step| step.kind = StepKind::Tool { tool: Some(tool) });
  let lint = edit.add_step(name("Lint"), Some(&checks));
  edit.update(&lint, |step| step.kind = StepKind::Tool { tool: Some(lint_tool) });
  let id = db
    .create_resource(
      Slug::new("problems").expect("slug"),
      name("Problems"),
      &edit.validate().expect("a pipeline"),
      Timestamp::now(),
    )
    .expect("the pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now()).expect("published");
  (id, reviewer, [review, checks, lint])
}

#[test]
fn open_builder_stands_under_the_agent_label_and_opens_the_profile() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  let builder = match &super::pipeline_step::stored(&agents, ready).steps()[0].kind {
    StepKind::Agent { profile, .. } => profile.expect("a profile"),
    StepKind::Tool { .. } => panic!("an agent step"),
  };
  open_step(&mut agents, ready, &plan);
  assert_eq!(agents.text("pipeline-hub-step-agent-row-label"), "Agent");
  assert_eq!(agents.text("pipeline-hub-step-open-profile-label"), "Open Builder");
  assert_eq!(
    agents.text("pipeline-hub-step-stopped-row-label"),
    "When the step fails"
  );
  assert_eq!(agents.text("pipeline-hub-step-stopped"), "Restart once, then ask");

  agents.click(LINK);
  agents.until("the profile", |s| route(s) == profile_page(builder));
}

#[test]
fn open_builder_over_unsaved_changes_asks_first() {
  let (mut agents, [ready, ..]) = setup();
  let plan = keys(&agents, ready)[0].clone();
  open_step(&mut agents, ready, &plan);
  agents.click("pipeline-hub-step-approval");
  agents.until("the edit holds it", |s| s.text(HINT).starts_with("Unsaved"));

  agents.click(LINK);
  agents.until("the question", |s| s.exists(KEEP));
  agents.click(KEEP);
  agents.until("still the step, its change kept", |s| {
    !s.exists(KEEP) && s.text(HINT).starts_with("Unsaved")
  });
  assert_eq!(route(&agents), super::pipeline_step::step_route(ready, &plan));

  agents.click(LINK);
  agents.until("the question again", |s| s.exists(DISCARD));
  agents.click(DISCARD);
  agents.until("the Agents hub", |s| {
    matches!(route(s), Route::Project(_, ProjectPage::AgentsHub(_)))
  });
}

#[test]
fn a_profile_with_only_a_draft_opens_too_and_a_tool_that_cannot_run_says_why() {
  let mut agents = Agents::new(true);
  let (id, reviewer, [review, checks, lint]) = problems(&agents);
  open_step(&mut agents, id, &review);
  assert_eq!(agents.text("pipeline-hub-step-open-profile-label"), "Open Reviewer");

  agents.click(&format!("pipeline-hub-step-chip-{checks}"));
  agents.until("the tool step", |s| s.exists("pipeline-hub-step-tool-unavailable"));
  assert!(!agents.exists(LINK), "a tool has no profile to open");
  assert_eq!(
    agents.text("pipeline-hub-step-agent-row-sub"),
    "Its exit code decides the step"
  );
  let notice = agents.text("pipeline-hub-step-tool-unavailable");
  assert!(
    notice.contains("This tool cannot run as a pipeline step in this version.") && notice.contains("Database actions"),
    "{notice}"
  );
  assert!(
    !agents.exists("pipeline-hub-step-instructions-row"),
    "a tool has no prompt"
  );
  assert!(agents.exists("pipeline-hub-step-approval-row"));
  for row in [
    "pipeline-hub-step-access-row",
    "pipeline-hub-step-base-moves-row",
    "pipeline-hub-step-sources-row",
    "pipeline-hub-step-commands-row",
    "pipeline-hub-step-network-row",
    "pipeline-hub-step-ask-tools-row",
  ] {
    assert!(!agents.exists(row), "a tool step has no {row}");
  }
  assert!(agents.exists("pipeline-hub-step-stopped-row"), "Run keeps its rows");

  // A command tool that can run says nothing of the kind, and keeps its instructions.
  agents.click(&format!("pipeline-hub-step-chip-{lint}"));
  agents.until("the runnable tool step", |s| {
    s.exists("pipeline-hub-step-instructions-row")
  });
  assert!(!agents.exists("pipeline-hub-step-tool-unavailable"));
  assert_eq!(
    agents.text("pipeline-hub-step-agent-row-sub"),
    "Its exit code decides the step"
  );

  agents.click(&format!("pipeline-hub-step-chip-{review}"));
  agents.until("the profile step", |s| s.exists(LINK));
  agents.click(LINK);
  agents.until("the draft profile", |s| route(s) == profile_page(reviewer));
}
