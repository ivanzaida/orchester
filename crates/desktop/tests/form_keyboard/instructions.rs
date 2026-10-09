//! Instructions (05.01 `yRKeF`): the sidebar opens the screen, New instruction creates a draft,
//! the prompt is saved as it is typed, Publish v1 publishes it, and the preview, the resolved
//! prompt and the variable catalog show the prompt it takes part in, for a new run or a recorded
//! one.

use std::{
  collections::{BTreeMap, BTreeSet},
  path::Path,
  time::Duration,
};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Instruction, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Slug,
    Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{github::shows, harness::Desktop};

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
    limits: TaskLimits::new(
      Some(TokenBudget::new(1_000_000).expect("budget")),
      Some(Duration::from_secs(1_800)),
    )
    .expect("limits"),
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task");
}

fn revision(hex_digit: char) -> Revision {
  Revision::new(&hex_digit.to_string().repeat(40)).expect("valid revision")
}

/// Two cancelled runs of the seeded task, both started from `aaaa…`: run 1 recorded a checkpoint
/// commit `bbbb…` on Implement, run 2 none.
fn seed_runs(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let task = db.tasks().expect("tasks").remove(0);
  let pipeline = task.details().pipeline.expect("the task has a pipeline");
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("read")
    .expect("published");
  let mut profiles = BTreeMap::new();
  for resource in db.resources(ResourceKind::AgentProfile).expect("profiles") {
    let profile = db.latest_version::<AgentProfile>(resource.id()).expect("read");
    profiles.insert(resource.id(), profile.expect("published"));
  }
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &std::collections::BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(pipeline, &version, published).expect("snapshot");
  for (number, checkpoint) in [(1, Some(revision('b'))), (2, None)] {
    let new = NewRun {
      task: task.id(),
      snapshot: snapshot.clone(),
      branch: BranchName::new(&format!("orchester/orc-1-run-{number}")).expect("branch"),
      base: revision('a'),
      base_branch: None,
      budget: Some(TokenBudget::new(1_000_000).expect("budget")),
      max_duration: Some(Duration::from_secs(1_800)),
    };
    let now = Timestamp::now();
    let mut run = db.create_run(IdempotencyKey::generate(), new, now).expect("run");
    let mut events = run.start(now).expect("start");
    let implement = run
      .latest_execution(&StepKey::new("implement").expect("key"))
      .expect("Implement is ready")
      .id();
    events.extend(run.start_step(implement, now).expect("start Implement"));
    if let Some(checkpoint) = checkpoint {
      events.extend(run.record_checkpoint(implement, checkpoint, now).expect("checkpoint"));
    }
    events.extend(run.cancel(now).expect("cancel"));
    db.append_events(&run, &events).expect("append");
  }
}

/// Chooses the preview's run with the keyboard, `down` options below the chosen one ("New run",
/// then the newest run). Pressing the select focuses it; Escape closes the menu it opened.
fn choose_run(desktop: &mut Desktop, down: usize) {
  desktop.wait_for("instruction-preview-run");
  desktop.press("instruction-preview-run");
  desktop.key("Escape", "Escape");
  desktop.enter();
  for _ in 0..down {
    desktop.key("ArrowDown", "ArrowDown");
  }
  desktop.enter();
}

/// The catalog's Run group with `variable` chosen.
fn show_variable(desktop: &mut Desktop, variable: &str) {
  desktop.click("instruction-variables");
  desktop.wait_for("variable-catalog");
  desktop.click("variables-run");
  let row = format!("variable-{variable}");
  desktop.wait_for(&row);
  desktop.click(&row);
  desktop.wait_for("variable-details");
}

fn close_catalog(desktop: &mut Desktop) {
  desktop.click("variable-catalog-close");
  desktop.settle("the catalog closed", |tree| {
    tree.get_element_by_id("variable-catalog").is_none()
  });
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

#[test]
fn instructions_run_revision_is_the_chosen_runs_latest_checkpoint() {
  let mut desktop = Desktop::with_runner(
    "no-runner.exe".into(),
    |repo| std::fs::create_dir_all(repo.join(".git")).expect("create .git"),
    |repo| {
      seed(repo);
      seed_runs(repo);
    },
  );
  create_rule(&mut desktop);
  desktop.click("instruction-prompt");
  desktop.type_text("Look at {{run.revision}}");
  let (base, checkpoint) = (revision('a'), revision('b'));

  // Run 2 has no checkpoint: the rule keeps its placeholder and the catalog says why.
  choose_run(&mut desktop, 1);
  assert!(shows(&mut desktop, "instruction-preview-run", "Run 002"));
  desktop.click("instruction-source-1");
  desktop.wait_for("instruction-viewer");
  assert!(shows(&mut desktop, "instruction-viewer", "Look at {{run.revision}}"));
  assert!(shows(&mut desktop, "instruction-viewer", "no value yet: run.revision"));
  desktop.click("instruction-viewer-close");
  show_variable(&mut desktop, "run.revision");
  assert!(shows(&mut desktop, "variable-details", "Unavailable · no commit yet"));
  assert!(shows(&mut desktop, "variable-details", "After the first commit"));
  desktop.click("variable-run.base_revision");
  assert!(shows(&mut desktop, "variable-details", base.as_str()));
  close_catalog(&mut desktop);

  // Run 1 committed a checkpoint: that is the revision, in the catalog and in the prompt.
  choose_run(&mut desktop, 1);
  assert!(shows(&mut desktop, "instruction-preview-run", "Run 001"));
  show_variable(&mut desktop, "run.revision");
  assert!(shows(&mut desktop, "variable-details", checkpoint.as_str()));
  close_catalog(&mut desktop);
  desktop.click("instruction-source-1");
  desktop.wait_for("instruction-viewer");
  assert!(shows(
    &mut desktop,
    "instruction-viewer",
    &format!("Look at {}", checkpoint.as_str())
  ));
}
