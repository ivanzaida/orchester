//! Tools (06.01 `ZFwWl`, 06.02 `YEWFo`) headlessly: the library lists the project's tools with
//! the kinds that cannot be created yet unavailable, a new command tool is saved as a draft and
//! published, a value the rules reject keeps Save draft disabled, and a tool step in the pipeline
//! editor can choose a published command tool but not a tool it cannot run.

use std::{
  collections::{BTreeMap, BTreeSet},
  path::Path,
};

use orchester_core::{
  id::{ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AllowedSteps, ApprovalMode, CommandAction, CommandArg, CommandProgram, DatabaseQuery,
    DeniedBehavior, Pipeline, PromptText, ResourceKind, Slug, Step, StepKey, StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{has, set_text},
};

/// The step inspector's profile or tool select.
const SELECT: &str = "step-executor-select";

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

/// A published command tool "Run test suite" and a published database tool, which this version
/// cannot run. Returns their ids.
fn seed_tools(repo: &Path) -> (ResourceId, ResourceId) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let now = Timestamp::now();
  let command = CommandAction::new(
    CommandProgram::new("cargo").expect("program"),
    vec![CommandArg::new("test").expect("argument")],
  );
  let suite = db
    .create_resource(
      Slug::new("run-test-suite").expect("slug"),
      name("Run test suite"),
      &Tool::command("Execute the project’s verification commands", command),
      now,
    )
    .expect("tool")
    .id();
  db.publish_draft::<Tool>(suite, now).expect("publish");
  let policy = AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    name("Staging"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy");
  let query = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).expect("query")),
    Some(policy),
  )
  .expect("tool");
  let database = db
    .create_resource(Slug::new("query").expect("slug"), name("Query staging"), &query, now)
    .expect("tool")
    .id();
  db.publish_draft::<Tool>(database, now).expect("publish");
  (suite, database)
}

fn open_tools(desktop: &mut Desktop) {
  desktop.wait_for("nav-tools");
  desktop.click("nav-tools");
  desktop.wait_for("tools-library");
}

fn tools(repo: &Path) -> Vec<(String, Option<u32>, Option<Tool>)> {
  let db = ProjectDb::open(repo).expect("open the project");
  db.resources(ResourceKind::Tool)
    .expect("tools")
    .into_iter()
    .map(|resource| {
      let latest = db.latest_version::<Tool>(resource.id()).expect("version");
      let draft = db.draft::<Tool>(resource.id()).expect("draft");
      (
        resource.name().as_str().to_owned(),
        latest.map(|version| version.number().get()),
        draft.map(|draft| draft.body().clone()),
      )
    })
    .collect()
}

#[test]
fn tools_library_lists_the_tools_and_only_command_can_be_created() {
  let mut desktop = Desktop::with_project();
  seed_tools(desktop.repo());
  open_tools(&mut desktop);
  desktop.settle("the tools", |tree| tree.get_element_by_id("tools-list").is_some());
  assert!(shows(&mut desktop, "tools-list", "Run test suite"));
  assert!(shows(
    &mut desktop,
    "tools-list",
    "Execute the project’s verification commands"
  ));
  assert!(shows(&mut desktop, "tools-list", "Ready"));
  assert!(shows(&mut desktop, "tools-list", "Query staging"));
  assert!(shows(&mut desktop, "tools-list", "Unavailable"));
  assert!(shows(&mut desktop, "tools-counts", "2 tools"));
  // The kinds that need secrets, approvals or GitHub push do nothing when pressed.
  for unavailable in [
    "tools-create-http",
    "tools-create-mcp",
    "tools-create-database",
    "tools-create-pull-request",
    "tools-import",
  ] {
    desktop.click(unavailable);
    assert!(has(&mut desktop, "tools-library"), "{unavailable} opened something");
  }
  desktop.click("tools-create-command");
  desktop.wait_for("tool-name");
}

#[test]
fn tools_new_command_tool_is_saved_as_a_draft_and_published_as_v1() {
  let mut desktop = Desktop::with_project();
  open_tools(&mut desktop);
  desktop.wait_for("tools-empty-add");
  desktop.click("tools-empty-add");
  desktop.wait_for("tool-name");
  // Without a name and a program there is nothing to save.
  desktop.click("tool-save-draft");
  assert!(tools(desktop.repo()).is_empty());
  set_text(&mut desktop, "tool-name", "Git version");
  set_text(&mut desktop, "tool-program", "git");
  set_text(&mut desktop, "tool-arguments", "--version");
  set_text(&mut desktop, "tool-exit-codes", "0, 1");
  set_text(&mut desktop, "tool-environment", "GIT_TRACE=0");
  desktop.click("tool-save-draft");
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved draft", |_| !tools(&repo).is_empty());
  desktop.settle("the editor of the saved tool", |tree| {
    tree
      .get_element_by_id("tool-save-status")
      .is_some_and(|status| status.text_content() == Some("Draft saved · Not published"))
  });
  let saved = tools(desktop.repo());
  assert_eq!(saved.len(), 1);
  let (tool_name, published, draft) = &saved[0];
  assert_eq!((tool_name.as_str(), *published), ("Git version", None));
  let draft = draft.as_ref().expect("a draft");
  let ToolAction::Command(command) = draft.action() else {
    panic!("a command tool");
  };
  assert_eq!(command.command_line(), "git --version");
  assert!(command.exit_codes().contains(1));
  assert_eq!(command.environment().iter().count(), 1);
  desktop.click("tool-publish");
  desktop.settle("the published tool", |tree| {
    tree
      .get_element_by_id("tool-save-status")
      .is_some_and(|status| status.text_content().is_some_and(|text| text == "Published v1"))
  });
  assert_eq!(tools(desktop.repo())[0].1, Some(1));
  assert!(tools(desktop.repo())[0].2.is_none(), "publishing removes the draft");
}

#[test]
fn tools_editor_says_why_a_value_cannot_be_used() {
  let mut desktop = Desktop::with_project();
  open_tools(&mut desktop);
  desktop.wait_for("tools-add");
  desktop.click("tools-add");
  desktop.wait_for("tool-name");
  set_text(&mut desktop, "tool-name", "Checks");
  set_text(&mut desktop, "tool-program", "../outside/check.sh");
  set_text(&mut desktop, "tool-timeout", "0");
  set_text(&mut desktop, "tool-environment", "NOT A LINE");
  assert!(shows(
    &mut desktop,
    "tool-page",
    "A relative program path must stay inside the worktree."
  ));
  assert!(shows(
    &mut desktop,
    "tool-page",
    "Enter whole seconds, from 1 up to 24 hours."
  ));
  assert!(shows(
    &mut desktop,
    "tool-page",
    "Write each variable as NAME=value: NOT A LINE"
  ));
  desktop.click("tool-save-draft");
  assert!(
    tools(desktop.repo()).is_empty(),
    "a tool with these values is not saved"
  );
  set_text(&mut desktop, "tool-program", "scripts/check.sh");
  set_text(&mut desktop, "tool-timeout", "30");
  set_text(&mut desktop, "tool-environment", "");
  assert!(!shows(
    &mut desktop,
    "tool-page",
    "A relative program path must stay inside the worktree."
  ));
  desktop.click("tool-save-draft");
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved draft", |_| !tools(&repo).is_empty());
}

/// A published pipeline with one tool step that has no tool yet.
fn seed_tool_step(repo: &Path) -> ResourceId {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let step = Step {
    key: StepKey::new("run-checks").expect("key"),
    name: name("Run checks"),
    kind: StepKind::Tool { tool: None },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  db.create_resource(
    Slug::new("checks").expect("slug"),
    name("Checks"),
    &pipeline,
    Timestamp::now(),
  )
  .expect("pipeline")
  .id()
}

fn step_tool(repo: &Path, pipeline: ResourceId) -> StepKind {
  let db = ProjectDb::open(repo).expect("open the project");
  let draft = db.draft::<Pipeline>(pipeline).expect("read").expect("draft");
  draft.body().steps()[0].kind.clone()
}

#[test]
fn tools_a_tool_step_chooses_a_published_command_tool_but_not_one_it_cannot_run() {
  let mut desktop = Desktop::with_project();
  let (suite, _) = seed_tools(desktop.repo());
  let pipeline = seed_tool_step(desktop.repo());
  desktop.wait_for("nav-pipelines");
  desktop.click("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.click("pipeline-row");
  desktop.wait_for("step-card-run-checks");
  desktop.press("step-card-run-checks");
  desktop.settle("the tool select", |tree| {
    tree
      .get_element_by_id_mut(SELECT)
      .and_then(|element| element.as_select())
      .is_some()
  });
  // Tool steps read no instructions and use no tokens.
  assert!(!has(&mut desktop, "step-instructions"));
  assert!(!has(&mut desktop, "step-budget"));
  let labels = select_labels(&mut desktop);
  let database = labels
    .iter()
    .position(|label| label.contains("Query staging"))
    .expect("the database tool is listed");
  commit(&mut desktop, database);
  assert_eq!(step_tool(desktop.repo(), pipeline), StepKind::Tool { tool: None });
  let command = labels
    .iter()
    .position(|label| label.contains("Run test suite"))
    .expect("the command tool is listed");
  commit(&mut desktop, command);
  let repo = desktop.repo().to_path_buf();
  desktop.settle("the saved choice", |_| {
    step_tool(&repo, pipeline) == StepKind::Tool { tool: Some(suite) }
  });
  desktop.wait_for("step-tool-command");
  assert!(shows(&mut desktop, "step-tool-command", "Runs cargo test"));
  desktop.click("step-open-tool");
  desktop.wait_for("tool-page");
  assert!(shows(&mut desktop, "tool-title", "Run test suite"));
}

fn select_labels(desktop: &mut Desktop) -> Vec<std::sync::Arc<str>> {
  desktop
    .tree
    .get_element_by_id_mut(SELECT)
    .and_then(|element| element.as_select())
    .expect("the tool select")
    .labels()
}

fn commit(desktop: &mut Desktop, index: usize) {
  desktop
    .tree
    .get_element_by_id_mut(SELECT)
    .and_then(|element| element.as_select())
    .expect("the tool select")
    .commit(index);
  desktop.pass();
}
