//! Instructions (05.01 `yRKeF`): the sidebar opens the screen, New instruction creates a draft,
//! the prompt is saved as it is typed, Publish v1 publishes it, and the preview, the resolved
//! prompt and the variable catalog show the prompt it takes part in.

use std::{collections::BTreeSet, path::Path, time::Duration};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Instruction, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug,
    Step, StepKey, StepKind,
  },
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::harness::Desktop;

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

/// A published Builder profile, the published pipeline Delivery that runs it on Implement, and a
/// task on that pipeline.
fn seed(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let now = Timestamp::now();
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build it.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("publish");
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Implement the plan.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(delivery, now).expect("publish");
  let details = TaskDetails {
    title: name("Add GitHub sign-in"),
    brief: "Users sign in with GitHub.".into(),
    acceptance_criteria: Vec::new(),
    pipeline: Some(delivery),
    limits: Some(
      TaskLimits::new(TokenBudget::new(1_000_000).expect("budget"), Duration::from_secs(1_800)).expect("limits"),
    ),
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task");
}

fn seeded() -> Desktop {
  Desktop::with_runner(
    "no-runner.exe".into(),
    |repo| std::fs::create_dir_all(repo.join(".git")).expect("create .git"),
    seed,
  )
}

/// The project's only instruction, as stored: its draft text and its latest version number.
fn stored(repo: &Path) -> Option<(String, Option<u32>)> {
  let db = ProjectDb::open(repo).expect("open");
  let resource = db
    .resources(ResourceKind::Instruction)
    .expect("resources")
    .into_iter()
    .next()?;
  let draft = db.draft::<Instruction>(resource.id()).expect("draft");
  let latest = db.latest_version::<Instruction>(resource.id()).expect("version");
  let text = draft
    .map(|draft| draft.body().text().as_str().to_owned())
    .or_else(|| latest.as_ref().map(|version| version.body().text().as_str().to_owned()))
    .unwrap_or_default();
  Some((text, latest.map(|version| version.number().get())))
}

/// Opens Instructions and creates the rule "Git conventions".
fn create_rule(desktop: &mut Desktop) {
  desktop.wait_for("nav-instructions");
  desktop.click("nav-instructions");
  desktop.wait_for("empty-new-instruction");
  desktop.click("empty-new-instruction");
  desktop.wait_for("new-instruction-name");
  desktop.click("new-instruction-name");
  desktop.type_text("Git conventions");
  desktop.click("new-instruction-create");
  desktop.wait_for("instruction-prompt");
}

#[test]
fn instructions_new_rule_is_saved_as_typed_and_published() {
  let mut desktop = Desktop::with_project();
  create_rule(&mut desktop);
  let repo = desktop.repo().to_path_buf();
  assert_eq!(stored(&repo), Some((String::new(), None)));
  desktop.click("instruction-prompt");
  desktop.type_text("Keep commits small.");
  desktop.settle("the draft saved", |_| {
    stored(&repo).is_some_and(|(text, _)| text == "Keep commits small.")
  });
  desktop.settle("the save state shown", |tree| {
    tree
      .get_element_by_id("instruction-save-status")
      .and_then(|status| status.text_content().map(str::to_owned))
      .is_some_and(|status| status == "Draft saved · Not published")
  });
  desktop.click("instruction-publish");
  desktop.settle("v1 published", |_| {
    stored(&repo).is_some_and(|(_, version)| version == Some(1))
  });
  desktop.settle("the published state shown", |tree| {
    tree
      .get_element_by_id("instruction-save-status")
      .and_then(|status| status.text_content().map(str::to_owned))
      .is_some_and(|status| status == "Published v1")
  });
}

#[test]
fn instructions_preview_lists_the_sources_and_opens_the_resolved_prompt() {
  let mut desktop = seeded();
  create_rule(&mut desktop);
  desktop.click("instruction-prompt");
  desktop.type_text("Commit on the branch.");
  // Profile, the rule, the step and the task.
  desktop.wait_for("instruction-source-3");
  desktop.click("instruction-view-prompt");
  desktop.wait_for("instruction-viewer");
  desktop.wait_for("instruction-viewer-source-3");
  desktop.click("instruction-viewer-close");
  desktop.settle("the viewer closed", |tree| {
    tree.get_element_by_id("instruction-viewer").is_none()
  });
}

#[test]
fn instructions_variable_catalog_inserts_the_chosen_expression() {
  let mut desktop = seeded();
  create_rule(&mut desktop);
  desktop.click("instruction-prompt");
  desktop.type_text("Work on");
  desktop.click("instruction-variables");
  desktop.wait_for("variable-catalog");
  desktop.click("variable-search");
  desktop.type_text("working branch");
  desktop.wait_for("variable-run.branch");
  assert!(desktop.tree.get_element_by_id("variable-task.title").is_none());
  desktop.click("variable-run.branch");
  desktop.wait_for("variable-details");
  desktop.click("variable-insert");
  desktop.settle("the catalog closed", |tree| {
    tree.get_element_by_id("variable-catalog").is_none()
  });
  assert_eq!(desktop.value("instruction-prompt"), "Work on {{run.branch}}");
}
