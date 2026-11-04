//! A run's own model and effort, changed from its Conversation (ORC-322): for the step at work alone or also for the
//! later steps on its connection, recorded for the Conversation's event lines; a change made while the step's turn runs
//! waits for that turn's end and can be canceled until then; a model or effort the connection does not offer is
//! refused, and a connection with nothing to choose from shows as fixed. The profile never changes.

mod support;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  name::Name,
  resource::{AgentProfile, ModelId, ResourceKind, StepKey},
  run::{Answerer, ModelChoice, ModelEventKind, ModelScope, Run, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::tasks::{
  detail::TaskDetail,
  run_model::{
    StepModel,
    back::BackToProfile,
    change::{ModelAsked, ModelChangeError, cancel_model_change_in, change_model_in},
  },
};
use support::{Fixture, at};

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("model")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn choice(id: &str, effort: Option<&str>) -> ModelChoice {
  ModelChoice {
    model: model(id),
    effort: effort.map(str::to_owned),
  }
}

fn efforts(levels: &[&str]) -> Vec<String> {
  levels.iter().map(|level| (*level).to_owned()).collect()
}

/// Keeps the connection Builder's profile names as a checked Codex that lists `models`.
fn connect(fixture: &mut Fixture, models: Vec<CatalogModel>) {
  let builder = builder_connection(fixture);
  let mut connection = AgentConnection::new(
    builder,
    Name::new("Codex · Work").expect("name"),
    AgentProvider::CodexCli,
    fixture.repo.path().join("bin").join("codex.exe"),
    at(1),
  )
  .expect("absolute executable");
  connection
    .record_check(ConnectionCheck {
      at: at(1),
      result: CheckResult::Available {
        version: "0.159.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, models),
      },
    })
    .expect("an agent check fits a CLI connection");
  fixture.db.create_connection(&connection).expect("kept");
}

fn builder_connection(fixture: &Fixture) -> orchester_core::id::ConnectionId {
  let profile = fixture
    .db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Builder")
    .expect("Builder");
  fixture
    .db
    .latest_version::<AgentProfile>(profile.id())
    .expect("read")
    .expect("published")
    .body()
    .connection()
}

/// The two models of the fixtures' Codex: the profile's `gpt-5.5` and `gpt-5.5-codex`.
fn two_models() -> Vec<CatalogModel> {
  vec![
    CatalogModel::new(model("gpt-5.5"), efforts(&["low", "medium", "high"])),
    CatalogModel::new(model("gpt-5.5-codex"), efforts(&["low", "medium", "high", "xhigh"])),
  ]
}

/// A started run of a new task, on a fixture whose Builder lists `models`.
fn started(mut fixture: Fixture, models: Vec<CatalogModel>) -> (Fixture, Run) {
  connect(&mut fixture, models);
  let task = fixture.queued_task("Add login", at(40));
  let run = fixture.started_run(task, at(50));
  (fixture, run)
}

fn asked(scope: ModelScope) -> ModelAsked {
  ModelAsked {
    step: key("implement"),
    to: choice("gpt-5.5-codex", Some("high")),
    scope,
  }
}

fn shown(fixture: &Fixture, run: &Run) -> StepModel {
  let detail = TaskDetail::load_in(fixture.repo.path(), run.task()).expect("detail");
  detail.run.expect("the run").models.step.expect("the step at work")
}

#[test]
fn run_model_change_for_the_step_holds_for_it_alone_and_is_recorded() {
  let (fixture, run) = started(Fixture::parallel(), two_models());
  let before = shown(&fixture, &run);
  assert_eq!(before.now, choice("gpt-5.5", None));
  assert!(!before.changed() && !before.fixed());
  assert_eq!(
    before.later.changing.len(),
    2,
    "Research and Docs run on Builder's connection"
  );

  change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Step), Answerer::Person).expect("changed");

  assert_eq!(
    fixture.db.step_model(run.id(), &key("implement")).expect("read"),
    Some(choice("gpt-5.5-codex", Some("high")))
  );
  assert_eq!(fixture.db.step_model(run.id(), &key("research")).expect("read"), None);
  let events = fixture.db.model_events(run.id()).expect("events");
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].by, Answerer::Person);
  let ModelEventKind::Changed {
    scope, to, was, also, ..
  } = &events[0].kind
  else {
    panic!("a change: {events:?}");
  };
  assert_eq!(
    (scope, to, was, also.len()),
    (
      &ModelScope::Step,
      &choice("gpt-5.5-codex", Some("high")),
      &choice("gpt-5.5", None),
      0
    )
  );
  let after = shown(&fixture, &run);
  assert_eq!(after.now, choice("gpt-5.5-codex", Some("high")));
  assert_eq!(after.profile, choice("gpt-5.5", None), "the profile stays");
  assert!(after.changed());
  assert_eq!(after.waiting, None, "no turn runs: it applies from the next");
  let detail = TaskDetail::load_in(fixture.repo.path(), run.task()).expect("detail");
  assert_eq!(
    detail.run.expect("run").steps[0].model.as_deref(),
    Some("gpt-5.5-codex")
  );
  let StepExecutor::Agent { profile, .. } = run.snapshot().steps()[0].executor() else {
    panic!("an agent step");
  };
  let pinned = fixture
    .db
    .version::<AgentProfile>(profile.resource, profile.number)
    .expect("read")
    .expect("kept");
  assert_eq!(
    pinned.body().model(),
    &model("gpt-5.5"),
    "the profile's model is untouched"
  );
}

#[test]
fn run_model_change_for_the_rest_of_the_run_reaches_later_steps_on_the_same_connection() {
  let (fixture, run) = started(Fixture::parallel(), two_models());
  change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Run), Answerer::Person).expect("changed");
  for step in ["implement", "research", "docs"] {
    assert_eq!(
      fixture.db.step_model(run.id(), &key(step)).expect("read"),
      Some(choice("gpt-5.5-codex", Some("high"))),
      "{step}"
    );
  }
  let events = fixture.db.model_events(run.id()).expect("events");
  let ModelEventKind::Changed { scope, also, .. } = &events[0].kind else {
    panic!("a change: {events:?}");
  };
  assert_eq!(scope, &ModelScope::Run);
  assert_eq!(also, &[key("research"), key("docs")]);
}

#[test]
fn run_model_change_for_the_rest_of_the_run_keeps_a_step_on_another_connection() {
  let (fixture, run) = started(Fixture::reviewed(), two_models());
  let before = shown(&fixture, &run);
  assert_eq!(before.later.changing.len(), 0);
  assert_eq!(before.later.keeping, [key("review")]);
  change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Run), Answerer::Person).expect("changed");
  assert_eq!(fixture.db.step_model(run.id(), &key("review")).expect("read"), None);
}

#[test]
fn run_model_change_while_a_turn_runs_waits_for_its_end_and_can_be_canceled() {
  let (mut fixture, run) = started(Fixture::new(), two_models());
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let mut turn = fixture
    .db
    .start_invocation(run.id(), execution, &model("gpt-5.5"), at(60))
    .expect("a turn runs");
  let root = fixture.repo.path().to_path_buf();

  change_model_in(&root, run.id(), asked(ModelScope::Step), Answerer::Person).expect("changed");
  let waiting = shown(&fixture, &run);
  assert_eq!(waiting.waiting, Some(choice("gpt-5.5-codex", Some("high"))));
  assert_eq!(
    waiting.now,
    choice("gpt-5.5-codex", Some("high")),
    "what the next turn runs on"
  );

  cancel_model_change_in(&root, run.id(), &key("implement"), Answerer::Person).expect("canceled");
  assert_eq!(fixture.db.step_model(run.id(), &key("implement")).expect("read"), None);
  let events = fixture.db.model_events(run.id()).expect("events");
  assert_eq!(
    events[1].kind,
    ModelEventKind::Canceled {
      stays: choice("gpt-5.5", None)
    }
  );
  assert_eq!(shown(&fixture, &run).waiting, None);
  assert!(matches!(
    cancel_model_change_in(&root, run.id(), &key("implement"), Answerer::Person),
    Err(ModelChangeError::NotWaiting)
  ));

  // Once the turn ended, a change applies from the next turn: nothing waits, nothing to cancel.
  change_model_in(&root, run.id(), asked(ModelScope::Step), Answerer::Person).expect("changed again");
  turn.ended_at = Some(Timestamp::now());
  turn.outcome = Some(orchester_core::connection::stream::InvocationOutcome::Completed);
  fixture.db.save_invocation(&turn).expect("the turn ended");
  assert_eq!(shown(&fixture, &run).waiting, None);
  assert!(matches!(
    cancel_model_change_in(&root, run.id(), &key("implement"), Answerer::Person),
    Err(ModelChangeError::NotWaiting)
  ));
  assert_eq!(
    fixture.db.step_model(run.id(), &key("implement")).expect("read"),
    Some(choice("gpt-5.5-codex", Some("high")))
  );
}

#[test]
fn run_model_refuses_what_the_connection_does_not_offer_and_records_no_change_of_nothing() {
  let (fixture, run) = started(Fixture::new(), two_models());
  let root = fixture.repo.path();
  let unknown = ModelAsked {
    to: choice("gpt-9", None),
    ..asked(ModelScope::Step)
  };
  assert!(matches!(
    change_model_in(root, run.id(), unknown, Answerer::Person),
    Err(ModelChangeError::NotOffered { .. })
  ));
  let effort = ModelAsked {
    to: choice("gpt-5.5", Some("xhigh")),
    ..asked(ModelScope::Step)
  };
  assert!(matches!(
    change_model_in(root, run.id(), effort, Answerer::Person),
    Err(ModelChangeError::NotOffered { .. })
  ));
  let same = ModelAsked {
    to: choice("gpt-5.5", None),
    ..asked(ModelScope::Step)
  };
  change_model_in(root, run.id(), same, Answerer::Person).expect("what the profile runs on already");
  assert!(fixture.db.model_events(run.id()).expect("events").is_empty());
  let effort_only = ModelAsked {
    to: choice("gpt-5.5", Some("high")),
    ..asked(ModelScope::Step)
  };
  change_model_in(root, run.id(), effort_only.clone(), Answerer::Person).expect("the effort changes");
  change_model_in(root, run.id(), effort_only, Answerer::Person).expect("nothing to change");
  let events = fixture.db.model_events(run.id()).expect("events");
  assert_eq!(events.len(), 1, "a change to what it runs on already is not recorded");
  assert!(events[0].kind.changes_effort_only());
}

#[test]
fn run_model_on_a_connection_with_one_model_and_no_effort_is_fixed() {
  let (fixture, run) = started(Fixture::new(), vec![CatalogModel::new(model("gpt-5.5"), Vec::new())]);
  assert!(shown(&fixture, &run).fixed());
  assert!(matches!(
    change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Step), Answerer::Person),
    Err(ModelChangeError::NotOffered { .. })
  ));
}

/// Starts the agent of step `step` of `run` `after` milliseconds from now, later than any change made so far.
fn start_later(fixture: &mut Fixture, run: &Run, step: &str, after: i64) -> orchester_core::id::StepExecutionId {
  let execution = run
    .current_attempt()
    .expect("attempt")
    .executions()
    .iter()
    .find(|execution| execution.step() == &key(step))
    .expect("the step's execution")
    .id();
  let at = Timestamp::from_unix_millis(Timestamp::now().unix_millis() + after);
  fixture
    .db
    .start_invocation(run.id(), execution, &model("gpt-5.5"), at)
    .expect("its agent starts");
  execution
}

fn back(fixture: &Fixture, run: &Run) -> Vec<BackToProfile> {
  let detail = TaskDetail::load_in(fixture.repo.path(), run.task()).expect("detail");
  detail.run.expect("the run").models.back
}

#[test]
fn run_model_next_step_after_a_change_for_one_step_starts_on_its_profile() {
  let (mut fixture, run) = started(Fixture::parallel(), two_models());
  change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Step), Answerer::Person).expect("changed");
  let research = start_later(&mut fixture, &run, "research", 1_000);
  start_later(&mut fixture, &run, "docs", 2_000);
  let lines = back(&fixture, &run);
  assert_eq!(lines.len(), 1, "only the first later step to start: {lines:?}");
  assert_eq!(
    (
      &lines[0].execution,
      &lines[0].step,
      &lines[0].changed,
      &lines[0].profile
    ),
    (&research, &key("research"), &key("implement"), &choice("gpt-5.5", None))
  );
}

#[test]
fn run_model_later_step_a_change_for_the_rest_of_the_run_reached_says_nothing_of_its_profile() {
  let (mut fixture, run) = started(Fixture::parallel(), two_models());
  change_model_in(fixture.repo.path(), run.id(), asked(ModelScope::Run), Answerer::Person).expect("changed");
  start_later(&mut fixture, &run, "research", 1_000);
  assert_eq!(back(&fixture, &run), Vec::new());
}
