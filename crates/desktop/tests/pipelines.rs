extern crate lurq_legacy as lurq;

use std::{fs, path::Path};

use orchester_core::{
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, DraftSaved, EditProblem, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug,
    StarterStep, StarterStepText, StepKey, StepKind,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_desktop::{
  drafts::save_draft_in,
  i18n::{FALLBACK_LOCALE, LOCALES},
  pipelines::{
    data::{OpenedPipeline, publish_in},
    editor::{EditorState, SaveState},
    fields::{Executor, RouteChoice, unused_step_name},
    layout::layout,
    library::{CreateError, NewPipeline, StartFrom, StarterText, VersionState, create, library},
    problems::publish_error,
  },
};
use orchester_store::ProjectDb;
use serde_json::Value;
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

/// The starter's names and instructions as the English locale has them, and the profile or
/// tool names it looks for.
fn starter_text() -> StarterText {
  let locale: Value = serde_json::from_str(LOCALES[0].1).expect("locale is JSON");
  let starter = &locale["pipelines"]["starter"];
  StarterText::new(|step| {
    let entry = &starter[locale_key(step)];
    let text = |field: &str| entry[field].as_str().expect("starter text is a string").to_owned();
    let step_text = StarterStepText {
      name: name(&text("name")),
      instructions: PromptText::new(&text("instructions")).expect("starter instructions have no placeholders"),
    };
    (step_text, text("uses"))
  })
}

const fn locale_key(step: StarterStep) -> &'static str {
  match step {
    StarterStep::Research => "research",
    StarterStep::Plan => "plan",
    StarterStep::Implement => "implement",
    StarterStep::RunChecks => "runChecks",
    StarterStep::Review => "review",
    StarterStep::CreatePullRequest => "createPullRequest",
  }
}

fn new_pipeline(pipeline_name: &str, start: StartFrom) -> NewPipeline {
  NewPipeline {
    name: name(pipeline_name),
    start,
    starter: starter_text(),
  }
}

fn publish_profile(db: &mut ProjectDb, role: &str) {
  let profile = AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-a").expect("valid model"),
    ProviderSettings::default(),
  );
  let slug = Slug::from_name(role).expect("role names give slugs");
  let resource = db
    .create_resource(slug, name(role), &profile, now())
    .expect("create profile");
  db.publish_draft::<AgentProfile>(resource.id(), now())
    .expect("publish profile");
}

fn opened(repo: &Path, id: orchester_core::id::ResourceId) -> OpenedPipeline {
  OpenedPipeline::load_in(repo, id).expect("open the pipeline")
}

#[test]
fn starter_creates_draft_v1_using_the_project_profiles_it_finds() {
  let (repo, mut db) = project_db();
  publish_profile(&mut db, "Explorer");
  publish_profile(&mut db, "Builder");
  let id = create(&mut db, new_pipeline("Feature delivery", StartFrom::Starter), now()).expect("create");
  let pipeline = opened(repo.path(), id);
  assert_eq!(&*pipeline.slug, "feature-delivery");
  assert!(pipeline.latest.is_none());
  let unassigned: Vec<&str> = pipeline.body.unassigned_steps().map(StepKey::as_str).collect();
  assert_eq!(unassigned, ["plan", "run-checks", "review", "create-pull-request"]);
  let research = pipeline.body.step(&key("research")).expect("research");
  assert_eq!(research.name.as_str(), "Research");
  assert!(matches!(research.kind, StepKind::Agent { profile: Some(_), .. }));

  let entries = library(&db).expect("library");
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].state, VersionState::DraftOnly);
  assert_eq!(entries[0].step_count, 6);
  let stages: Vec<Vec<&str>> = entries[0]
    .stages
    .iter()
    .map(|stage| stage.iter().map(|name| &**name).collect())
    .collect();
  assert_eq!(
    stages,
    [
      vec!["Research"],
      vec!["Plan"],
      vec!["Implement"],
      vec!["Run checks", "Review"],
      vec!["Create pull request"]
    ]
  );
}

#[test]
fn a_name_whose_slug_is_taken_is_rejected_with_the_pipeline_named() {
  let (_repo, mut db) = project_db();
  create(&mut db, new_pipeline("Feature delivery", StartFrom::Empty), now()).expect("first");
  let again = create(&mut db, new_pipeline("Feature  Delivery!", StartFrom::Starter), now());
  assert!(matches!(
    again,
    Err(CreateError::SlugTaken { slug, existing }) if slug == "feature-delivery" && existing == "Feature delivery"
  ));
  assert!(matches!(
    create(&mut db, new_pipeline("Сборка", StartFrom::Empty), now()),
    Err(CreateError::NoSlug)
  ));
  assert_eq!(library(&db).expect("library").len(), 1);
}

#[test]
fn edits_save_as_the_draft_and_publish_as_the_next_version() {
  let (repo, mut db) = project_db();
  for role in ["Explorer", "Planner", "Builder", "Reviewer"] {
    publish_profile(&mut db, role);
  }
  let id = create(&mut db, new_pipeline("Delivery", StartFrom::Empty), now()).expect("create");
  let mut state = EditorState::new(opened(repo.path(), id).into());
  assert_eq!(state.problems(), [EditProblem::NoSteps]);
  assert!(!state.can_publish());

  let profile = state.assignables().profiles[0].id;
  let added = state.apply(|edit| edit.add_step(name("Research"), None));
  state.apply(|edit| edit.update(&added, |step| Executor::Profile(profile).apply(step)));
  assert_eq!(state.save_state(), SaveState::Pending);
  let (revision, draft) = state.draft_to_save(now()).expect("a valid edit is saved");
  let outcome = save_draft_in(repo.path(), id, &draft).expect("save");
  assert_eq!(outcome, DraftSaved::Stored);
  state.saved(revision, now(), outcome);
  assert!(state.can_publish());

  let reopened = opened(repo.path(), id);
  assert_eq!(reopened.body.steps().len(), 1);
  let (version, at) = publish_in(repo.path(), id).expect("publish");
  assert_eq!(version.get(), 1);
  state.published(version, at, draft.body().clone());
  assert!(!state.has_draft());
  assert_eq!(state.draft_number(), 2);
  assert_eq!(state.save_state(), SaveState::Saved);

  state.apply(|edit| edit.update(&added, |step| step.approval_gate = true));
  let (revision, next) = state.draft_to_save(now()).expect("the next edit starts draft v2");
  assert_eq!(next.based_on(), Some(version));
  let outcome = save_draft_in(repo.path(), id, &next).expect("save v2 draft");
  state.saved(revision, now(), outcome);
  assert!(state.has_draft());

  // Undoing the edit leaves nothing to publish: the draft is removed, not kept as v2.
  state.apply(|edit| edit.update(&added, |step| step.approval_gate = false));
  let (revision, back) = state.draft_to_save(now()).expect("the undo is saved");
  let outcome = save_draft_in(repo.path(), id, &back).expect("save the undo");
  assert_eq!(outcome, DraftSaved::Discarded);
  state.saved(revision, now(), outcome);
  assert!(!state.has_changes());
  assert!(opened(repo.path(), id).draft_saved_at.is_none());
  assert_eq!(
    library(&db).expect("library")[0].state,
    VersionState::Published(version)
  );
}

#[test]
fn a_cycle_is_kept_on_screen_but_not_saved() {
  let (repo, mut db) = project_db();
  let id = create(&mut db, new_pipeline("Delivery", StartFrom::Starter), now()).expect("create");
  let mut state = EditorState::new(opened(repo.path(), id).into());
  state.apply(|edit| edit.set_dependency(&key("plan"), &key("review"), true));
  assert_eq!(state.save_state(), SaveState::Invalid);
  assert!(state.draft_to_save(now()).is_none());
  assert!(matches!(state.problems()[0], EditProblem::Cycle(_)));
  state.apply(|edit| edit.set_dependency(&key("plan"), &key("review"), false));
  assert_eq!(state.save_state(), SaveState::Pending);
}

#[test]
fn publishing_a_stale_draft_names_both_versions() {
  let (repo, mut db) = project_db();
  publish_profile(&mut db, "Explorer");
  let id = create(&mut db, new_pipeline("Delivery", StartFrom::Empty), now()).expect("create");
  let mut state = EditorState::new(opened(repo.path(), id).into());
  let profile = state.assignables().profiles[0].id;
  let step = state.apply(|edit| edit.add_step(name("Research"), None));
  state.apply(|edit| edit.update(&step, |target| Executor::Profile(profile).apply(target)));
  let (_, draft) = state.draft_to_save(now()).expect("valid");
  save_draft_in(repo.path(), id, &draft).expect("save");
  publish_in(repo.path(), id).expect("v1");
  // An editor opened before v1 was published still holds a draft based on nothing.
  save_draft_in(repo.path(), id, &draft).expect("save the old draft again");
  let error = publish_in(repo.path(), id).expect_err("stale");
  assert_eq!(publish_error(&error).key, "pipelines.problems.staleDraft");
}

#[test]
fn starter_layout_matches_the_design_positions() {
  let pipeline = starter_text().pipeline(&Default::default());
  let graph = layout(pipeline.steps(), None);
  let positions: Vec<(f32, f32)> = (0..6)
    .map(|index| {
      let card = graph.card(index).expect("every step is placed");
      (card.x, card.y)
    })
    .collect();
  assert_eq!(
    positions,
    [
      (40.0, 100.0),
      (336.0, 100.0),
      (632.0, 100.0),
      (184.0, 364.0),
      (632.0, 364.0),
      (408.0, 608.0)
    ]
  );
  assert_eq!(graph.parallel_labels, [(184.0, 330.0)]);
  assert_eq!(graph.join_labels, [(400.0, 563.0, 2)]);
  assert!(graph.cycle.is_none());
}

#[test]
fn a_cycle_is_drawn_around_the_graph() {
  let mut steps = starter_text().pipeline(&Default::default()).into_steps();
  steps[1].depends_on.insert(key("review"));
  let graph = layout(&steps, Some((1, 4)));
  let cycle = graph.cycle.clone().expect("the closing dependency is drawn");
  assert_eq!((cycle.waiting, cycle.dependency), (1, 4));
  assert_eq!(graph.card(1).map(|card| (card.x, card.y)), Some((336.0, 100.0)));
  assert_eq!(cycle.label, (600.0, 62.0));
}

#[test]
fn a_new_failure_route_allows_one_retry() {
  let mut step = starter_text().pipeline(&Default::default()).into_steps().remove(0);
  RouteChoice(Some(key("plan"))).apply(&mut step);
  assert_eq!(step.on_failure.as_ref().map(|route| route.max_retries.get()), Some(1));
  Executor::Unset.apply(&mut step);
  assert!(matches!(
    step.kind,
    StepKind::Agent {
      profile: None,
      repository: RepositoryAccess::ReadOnly,
      ..
    }
  ));
}

#[test]
fn every_pipeline_message_key_exists() {
  // The screens' own words, and the workspace's they show too.
  let locales: Vec<Value> = [LOCALES, orchester_workspace::i18n::LOCALES]
    .iter()
    .map(|locales| {
      let (_, fallback) = locales
        .iter()
        .find(|(locale, _)| *locale == FALLBACK_LOCALE)
        .expect("fallback");
      serde_json::from_str(fallback).expect("locale is JSON")
    })
    .collect();
  let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
  let mut missing = Vec::new();
  for dir in [
    "desktop/src/pipelines",
    "desktop/src/screens/project/pipelines",
    "desktop/src/screens/project/pipelines/editor",
    // The pipelines' data and wording moved to the workspace.
    "workspace/src/pipelines",
  ] {
    for entry in fs::read_dir(crates.join(dir)).expect("source directory") {
      let path = entry.expect("entry").path();
      if path.extension().is_none_or(|ext| ext != "rs") {
        continue;
      }
      let source = fs::read_to_string(&path).expect("source");
      for part in source.split("Message::new(\"").skip(1) {
        let (key, _) = part.split_once('"').expect("closed literal");
        let found = locales.iter().any(|locale| {
          key
            .split('.')
            .try_fold(locale, |value, segment| value.get(segment))
            .is_some_and(Value::is_string)
        });
        if !found {
          missing.push(key.to_owned());
        }
      }
    }
  }
  assert!(
    missing.is_empty(),
    "message keys missing from both en.json files: {missing:?}"
  );
}

#[test]
fn published_pipeline_reopens_without_a_draft() {
  let (repo, mut db) = project_db();
  publish_profile(&mut db, "Explorer");
  let id = create(&mut db, new_pipeline("Delivery", StartFrom::Empty), now()).expect("create");
  let state = EditorState::new(opened(repo.path(), id).into());
  let profile = state.assignables().profiles[0].id;
  let mut edit = state.edit().clone();
  let step = edit.add_step(name("Research"), None);
  edit.update(&step, |target| Executor::Profile(profile).apply(target));
  let body: Pipeline = edit.validate().expect("valid");
  save_draft_in(
    repo.path(),
    id,
    &orchester_core::resource::Draft::new(body, None, now()),
  )
  .expect("save");
  publish_in(repo.path(), id).expect("publish");
  let reopened = opened(repo.path(), id);
  assert!(reopened.draft_saved_at.is_none());
  assert_eq!(reopened.latest.map(|latest| latest.number.get()), Some(1));
  assert_eq!(
    library(&db).expect("library")[0].state,
    VersionState::Published(reopened.based_on.expect("v1"))
  );
}

#[test]
fn added_steps_get_names_no_step_has() {
  let mut steps = starter_text().pipeline(&Default::default()).into_steps();
  let numbered = |number: usize| format!("New step {number}");
  let first = unused_step_name(&steps, "New step", numbered);
  assert_eq!(first.as_str(), "New step");
  steps[0].name = first;
  steps[1].name = name("New step 2");
  assert_eq!(unused_step_name(&steps, "New step", numbered).as_str(), "New step 3");
}
