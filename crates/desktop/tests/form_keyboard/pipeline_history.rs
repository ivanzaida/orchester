//! Version history in the pipeline editor: the published versions newest first with their
//! changes and runs, a version shown read-only on the canvas, compared with the draft, and
//! restored as the draft — after a second click while the draft has changes of its own.

use std::{collections::BTreeMap, path::Path, time::Duration};

use lurq::app::events::{MouseButton, ScrollPhase};
use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::contains_text,
  harness::{Desktop, expected},
};

const HISTORY: &str = "pipeline-history";
const DETAILS: &str = "pipeline-version-details";
const STEP_NAME: &str = "step-name";

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn step(step_name: &str, profile: ResourceId) -> Step {
  Step {
    key: StepKey::new("implement").expect("valid key"),
    name: name(step_name),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// "Delivery": v1 runs "Implement" and was used by run #1, v2 renames it to "Build", and draft
/// v3 makes it wait for approval.
fn seed(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project database");
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(1))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(2))
      .expect("publish the profile"),
  )]);
  let first = Pipeline::new(vec![step("Implement", builder)]).expect("valid pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &first, at(3))
    .expect("pipeline")
    .id();
  let v1 = db.publish_draft::<Pipeline>(delivery, at(4)).expect("publish v1");
  let renamed = Pipeline::new(vec![step("Build", builder)]).expect("valid pipeline");
  db.save_draft(delivery, &Draft::new(renamed, Some(v1.number()), at(5)))
    .expect("save v2");
  let v2 = db.publish_draft::<Pipeline>(delivery, at(6)).expect("publish v2");
  let mut gated = step("Build", builder);
  gated.approval_gate = true;
  let gated = Pipeline::new(vec![gated]).expect("valid pipeline");
  db.save_draft(delivery, &Draft::new(gated, Some(v2.number()), at(7)))
    .expect("save draft v3");
  run_of(&mut db, delivery, &v1, &profiles);
}

/// A finished run that pinned `version`.
fn run_of(
  db: &mut ProjectDb,
  pipeline: ResourceId,
  version: &orchester_core::resource::Version<Pipeline>,
  profiles: &BTreeMap<ResourceId, orchester_core::resource::Version<AgentProfile>>,
) {
  let budget = TokenBudget::new(1_000_000).expect("above zero");
  let details = TaskDetails {
    title: name("Add switcher"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: Some(pipeline),
    limits: Some(TaskLimits::new(budget, Duration::from_secs(1_800)).expect("limits")),
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(8))
    .expect("task")[0]
    .id();
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles,
    tools: &tools,
    instructions: &instructions,
  };
  let new = NewRun {
    task,
    snapshot: RunSnapshot::build(pipeline, version, published).expect("snapshot"),
    branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    budget,
    max_duration: Duration::from_secs(1_800),
  };
  let mut run = db.create_run(IdempotencyKey::generate(), new, at(9)).expect("run");
  let events = run.start(at(9)).expect("start");
  db.append_events(&run, &events).expect("append");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(10)).expect("start step");
  db.append_events(&run, &events).expect("append");
  let events = run.complete_step(execution, at(11)).expect("complete step");
  db.append_events(&run, &events).expect("append");
}

/// The seeded pipeline open in its editor on draft v3.
fn editor() -> Desktop {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-pipelines");
  desktop.press("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.press("pipeline-row");
  desktop.wait_for(STEP_NAME);
  desktop
}

/// The editor on version history, which opens on the latest version.
fn history() -> Desktop {
  let mut desktop = editor();
  desktop.press("pipeline-toolbar-history");
  desktop.wait_for(DETAILS);
  desktop
}

fn shows(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

fn shown(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

fn back(desktop: &mut Desktop) {
  desktop.tree.mouse_down(720.0, 480.0, MouseButton::Other(4));
  desktop.pass();
  desktop.tree.mouse_up(720.0, 480.0, MouseButton::Other(4));
  desktop.pass();
}

#[test]
fn pipeline_history_opens_on_the_latest_version_with_its_changes_and_runs() {
  let mut desktop = history();
  assert!(shows(&mut desktop, "pipeline-version-2", "1 change · No runs"));
  assert!(shows(&mut desktop, "pipeline-version-1", "First version · 1 run"));
  assert!(shows(&mut desktop, DETAILS, "Changes from v1"));
  assert!(shows(&mut desktop, DETAILS, "Build · renamed from Implement"));
  assert!(shows(
    &mut desktop,
    "pipeline-version-runs",
    "No run pinned this version"
  ));
  assert!(shows(&mut desktop, "pipeline-state", "Viewing v2 · Read-only"));
  // The canvas shows v2; the draft's steps and their inspector are not shown.
  assert!(shows(&mut desktop, "step-card-implement", "Build"));
  assert!(!shows(&mut desktop, "step-card-implement", "Waits for your approval"));
  assert!(!shown(&mut desktop, STEP_NAME));
  assert!(!shown(&mut desktop, "pipeline-add-step"));
  // The history is an entry of its own: going back returns to the draft.
  back(&mut desktop);
  desktop.wait_for(STEP_NAME);
  assert!(!shown(&mut desktop, HISTORY));
}

#[test]
fn pipeline_history_shows_an_older_version_read_only() {
  let mut desktop = history();
  desktop.press("pipeline-version-1");
  desktop.settle("v1 on the canvas", |tree| {
    tree
      .get_element_by_id("step-card-implement")
      .is_some_and(|card| contains_text(card, "Implement"))
  });
  assert!(shows(&mut desktop, "pipeline-version-runs", "#1"));
  assert!(shows(&mut desktop, DETAILS, "This is the first version"));
  // A card of a published version selects nothing.
  desktop.press("step-card-implement");
  assert!(!shown(&mut desktop, STEP_NAME));
  assert!(shown(&mut desktop, HISTORY));
  // Closing the history returns to the draft.
  desktop.press("pipeline-history-close");
  desktop.wait_for(STEP_NAME);
  assert!(shows(&mut desktop, "step-card-implement", "Waits for your approval"));
}

#[test]
fn pipeline_history_compares_a_version_with_the_draft() {
  let mut desktop = history();
  desktop.press("pipeline-version-1");
  desktop.wait_for("pipeline-history-compare");
  desktop.press("pipeline-history-compare");
  desktop.settle("the comparison", |tree| {
    tree
      .get_element_by_id(DETAILS)
      .is_some_and(|details| contains_text(details, "From v1 to draft v3"))
  });
  assert!(shows(&mut desktop, DETAILS, "Build · renamed from Implement"));
  assert!(shows(&mut desktop, DETAILS, "Build · now waits for approval"));
  desktop.press("pipeline-history-compare");
  desktop.settle("the comparison hidden", |tree| {
    tree
      .get_element_by_id(DETAILS)
      .is_some_and(|details| contains_text(details, "This is the first version"))
  });
}

#[test]
fn pipeline_history_restore_replaces_the_draft_after_a_second_click() {
  let mut desktop = history();
  desktop.press("pipeline-version-1");
  desktop.wait_for("pipeline-history-restore");
  assert!(shows(&mut desktop, "pipeline-history-restore", "Restore v1 as draft"));
  desktop.press("pipeline-history-restore");
  assert!(shows(
    &mut desktop,
    "pipeline-history-restore",
    "Replace draft v3? Click again"
  ));
  assert!(shown(&mut desktop, HISTORY), "the first click only asks");
  desktop.press("pipeline-history-restore");
  desktop.wait_for(STEP_NAME);
  assert!(!shown(&mut desktop, HISTORY));
  assert_eq!(desktop.value(STEP_NAME), "Implement");
  let repo = desktop.repo().to_path_buf();
  desktop.settle("v1 stored as draft v3", |_| {
    let db = ProjectDb::open(&repo).expect("open");
    let delivery = db
      .resources(orchester_core::resource::ResourceKind::Pipeline)
      .expect("pipelines")[0]
      .id();
    let draft = db.draft::<Pipeline>(delivery).expect("read");
    let v1 = db
      .version::<Pipeline>(delivery, orchester_core::resource::VersionNumber::FIRST)
      .expect("read")
      .expect("v1");
    draft.is_some_and(|draft| draft.body() == v1.body() && draft.based_on().map(|number| number.get()) == Some(2))
  });
}

#[test]
fn pipeline_history_restoring_the_latest_version_discards_the_draft() {
  let mut desktop = history();
  desktop.press("pipeline-history-restore");
  desktop.press("pipeline-history-restore");
  desktop.wait_for(STEP_NAME);
  // The draft repeated v2, so none is kept: the editor shows v2 and offers its history.
  desktop.settle("no draft left", |tree| {
    tree
      .get_element_by_id("pipeline-version")
      .is_some_and(|label| contains_text(label, "Published v2"))
  });
  desktop.wait_for("pipeline-version-history");
  assert!(!shown(&mut desktop, "pipeline-toolbar-history"));
  desktop.press("pipeline-version-history");
  desktop.wait_for(DETAILS);
  assert!(shows(
    &mut desktop,
    "pipeline-history-compare",
    "Compare with the latest version"
  ));
  // Nothing to restore: the editor holds v2, so Restore does nothing.
  desktop.press("pipeline-history-restore");
  assert!(shown(&mut desktop, HISTORY));
  assert!(shows(&mut desktop, "pipeline-history-restore", "Restore v2 as draft"));
}

fn size(desktop: &mut Desktop, id: &str) -> (f32, f32) {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .map(|bounds| (bounds.width, bounds.height))
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn wheel(desktop: &mut Desktop, delta_y: f32) {
  let (x, y) = (700.0, 500.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, delta_y, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

/// A verifier saw the graph's area keep a smaller window's size in the version view after the
/// window grew; the area must always fill the canvas above the zoom controls' band.
#[test]
fn pipeline_history_canvas_follows_the_window_after_it_grows() {
  let mut desktop = editor();
  desktop.tree.resize(1200, 800);
  desktop.pass();
  wheel(&mut desktop, -120.0);
  desktop.press("pipeline-zoom-fit");
  desktop.tree.resize(1440, 960);
  desktop.pass();
  desktop.press("pipeline-zoom-full");
  desktop.press("pipeline-toolbar-history");
  desktop.wait_for("pipeline-history-compare");
  desktop.press("pipeline-history-compare");
  desktop.press("pipeline-version-1");
  desktop.wait_for(DETAILS);
  let (canvas, area) = (
    size(&mut desktop, "pipeline-canvas"),
    size(&mut desktop, "pipeline-canvas-area"),
  );
  assert_eq!(area.0, canvas.0);
  assert!(area.1 < canvas.1 && area.1 > canvas.1 / 2.0, "{area:?} in {canvas:?}");
}

/// Publish opens its confirmation focused, as the other dialogs without fields open: the field of
/// the inspector behind it loses the focus, and Tab starts at Cancel and stays in the dialog.
#[test]
fn publish_confirmation_takes_the_focus_when_it_opens() {
  let mut desktop = editor();
  desktop
    .tree
    .get_element_by_id_mut(STEP_NAME)
    .expect("the step name")
    .focus();
  desktop.pass();
  desktop.press("pipeline-publish");
  desktop.wait_for("publish-confirm");
  assert_eq!(desktop.focused().as_deref(), Some("publish-dialog"));
  let order = [Some("publish-cancel"), Some("publish-confirm"), Some("publish-cancel")];
  assert_eq!(desktop.stops(order.len(), false), expected(&order));
}
