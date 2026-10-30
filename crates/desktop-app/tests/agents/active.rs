//! The usage row must retain the run's pinned identity after profile and pipeline edits.
use std::collections::BTreeMap;

use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{AgentProfile, Draft, Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{harness::Agents, support::project_id};

#[test]
fn agents_active_run_keeps_pinned_identity_metadata_and_task_navigation() {
  let mut screen = Agents::new(true);
  let profile = screen.profile(true);
  let mut db = ProjectDb::open(&screen.root).expect("database");
  let snapshot = pipeline(&mut db, profile);
  let started = Timestamp::from_unix_millis(1_700_000_100_000);
  let run = running(&mut db, snapshot, started);
  republish(&mut db, &run, profile);
  let listed = orchester_workspace::agents::data::ProjectAgents::for_screen(&screen.root).expect("usage");
  let active = &listed.usage.active[0];
  assert_eq!(listed.usage.active.len(), 1, "the pending sibling is not active");
  assert_eq!(active.profile_name.as_deref(), Some("Builder"));
  assert_eq!(active.version.get(), 1);
  assert_eq!(active.pipeline_version.get(), 1);
  assert_eq!(active.position, 2, "position in the pinned pipeline, before filtering");
  assert_eq!(&*active.step, "Implement");
  assert_eq!(active.started_at, Some(started), "Started, not creation time");

  screen.open(profile);
  assert_eq!(screen.text("agent-active-count"), "1");
  let bounds = screen
    .window
    .tree
    .get_element_by_id_mut("agent-active-0")
    .and_then(|element| element.bounds())
    .expect("active row laid out");
  // A hub row (`fYNhS`): its label and the run's line under it.
  assert!(bounds.height >= 48.0, "the row holds both lines: {bounds:?}");
  let text = screen.text("agent-active-0");
  for expected in [
    "KON-1 · Test the pinned profile".to_owned(),
    "Delivery v1 · 02 Implement".to_owned(),
    format!("started {}", orchester_workspace::display::clock(started)),
    // The pinned version: the run keeps v1 of the profile, whatever it is called now.
    "Keeps v1 until it ends".to_owned(),
  ] {
    assert!(text.contains(&expected), "{expected}: {text}");
  }
  assert!(!text.contains("Renamed"));
  screen.click("agent-active-0");
  assert_eq!(
    screen.window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Task(active.task.clone(), TaskView::Summary))
  );
}

fn pipeline(db: &mut ProjectDb, profile: ResourceId) -> RunSnapshot {
  let steps = ["Other", "Implement"].map(|title| Step {
    key: StepKey::new(&title.to_lowercase()).expect("key"),
    name: Name::new(title).expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Work.").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  });
  let body = Pipeline::new(steps.into()).expect("pipeline");
  let resource = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("pipeline resource");
  let version = db
    .publish_draft::<Pipeline>(resource.id(), Timestamp::now())
    .expect("publish");
  RunSnapshot::build(
    resource.id(),
    &version,
    PublishedVersions {
      profiles: &BTreeMap::from([(
        profile,
        db.latest_version::<AgentProfile>(profile)
          .expect("profile")
          .expect("version"),
      )]),
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )
  .expect("snapshot")
}

fn running(db: &mut ProjectDb, snapshot: RunSnapshot, started: Timestamp) -> Run {
  let created = Timestamp::from_unix_millis(1_700_000_000_000);
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: Name::new("Test the pinned profile").expect("name"),
        brief: "Work.".into(),
        acceptance_criteria: vec!["Done.".into()],
        pipeline: Some(snapshot.pipeline().resource),
        limits: TaskLimits::NONE,
      }],
      created,
    )
    .expect("task")
    .remove(0);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot,
        branch: BranchName::new("orchester/test").expect("branch"),
        base: Revision::new(&"a".repeat(40)).expect("revision"),
        base_branch: None,
        base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      created,
    )
    .expect("run");
  let events = run.start(started).expect("start");
  db.append_events(&run, &events).expect("record start");
  let key = StepKey::new("implement").expect("key");
  let execution = run.latest_execution(&key).expect("execution").id();
  let events = run.start_step(execution, started).expect("step start");
  db.append_events(&run, &events).expect("record step");
  run
}

fn republish(db: &mut ProjectDb, run: &Run, profile: ResourceId) {
  let old = db
    .latest_version::<AgentProfile>(profile)
    .expect("profile")
    .expect("v1");
  let body = old.body();
  let renamed = AgentProfile::new(
    Name::new("Renamed").expect("name"),
    body.responsibilities().to_owned(),
    body.instructions().clone(),
    body.connection(),
    body.model().clone(),
    body.settings().clone(),
  )
  .with_permissions(body.permissions().clone());
  db.save_draft(profile, &Draft::new(renamed, Some(old.number()), Timestamp::now()))
    .expect("save profile");
  db.publish_draft::<AgentProfile>(profile, Timestamp::now())
    .expect("publish profile");
  let mut steps = run.snapshot().body().steps().to_vec();
  steps.reverse();
  steps[0].name = Name::new("Renamed step").expect("name");
  let pinned = run.snapshot().pipeline();
  db.save_draft(
    pinned.resource,
    &Draft::new(
      Pipeline::new(steps).expect("pipeline"),
      Some(pinned.number),
      Timestamp::now(),
    ),
  )
  .expect("save pipeline");
  db.publish_draft::<Pipeline>(pinned.resource, Timestamp::now())
    .expect("publish pipeline");
}
