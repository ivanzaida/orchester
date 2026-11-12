//! Pipeline setup gates use permission consent, independently of runtime approval answering. A local project's
//! pipeline is edited in the Pipelines hub's panel, whose *Save* publishes the next version at once and asks *Allow*
//! for what it widens: a step that stops waiting for a person's approval, or stops asking about its agent's tool calls.
//! Each is a switch of the step's panel.
use std::{
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{PipelineView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, ResourceKind, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  Served,
  http::post,
  kontur::{self, SeedRequest, scenarios},
};

/// What the person's edit widens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Widens {
  /// The first step stops waiting for the person's approval: its switch in the step's panel.
  Wait,
  /// The first step stops asking about its agent's tool calls: its switch in the step's panel.
  Ask,
}

struct Fixture {
  window: Served,
  repo: PathBuf,
  id: ResourceId,
  /// The pipeline as stored when the app started: its latest version.
  before: Pipeline,
  /// How many versions it has then.
  versions: usize,
  widens: Widens,
}

impl Fixture {
  fn new(widens: Widens) -> Self {
    let folder = tempfile::tempdir().expect("private fixture folder");
    let request = SeedRequest {
      data_dir: folder.path().join("bin"),
      repo: folder.path().join("repo-pipelines"),
      scenario: scenarios::named("pipelines").expect("pipeline scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("private seed");
    let db = ProjectDb::open(&seeded.repo).expect("private database");
    let id = db
      .resources(ResourceKind::Pipeline)
      .expect("resources")
      .into_iter()
      .find(|row| row.name().as_str() == "Narrowed permissions")
      .expect("narrowed fixture")
      .id();
    // Its one step waits for the person's approval and asks about its agent's tool calls.
    let before = db
      .latest_version::<Pipeline>(id)
      .expect("read version")
      .expect("published fixture")
      .body()
      .clone();
    let versions = db.versions::<Pipeline>(id).expect("versions").len();
    drop(db);
    let runner = folder.path().join("no-runner/orchester-runner");
    let mut window = Served::start_with(folder, &seeded.app_database, runner, true);
    window.until("navigation attached", |s| s.link.navigation().is_ok());
    let page = ProjectPage::Pipeline(id, PipelineView::Step(before.steps()[0].key.clone()));
    window
      .link
      .navigation()
      .expect("navigation")
      .go(Route::Project(seeded.project, page));
    window.until("the pipeline's panel", |s| s.shows("pipeline-hub-save"));
    assert!(
      !window
        .services()
        .workspace
        .automation_answers_approvals()
        .expect("approvals opt-in")
    );
    Self {
      window,
      repo: seeded.repo,
      id,
      before,
      versions,
      widens,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("private database")
  }

  /// Nothing was stored: no draft, no new version, the latest as it was.
  fn unchanged(&self) {
    let db = self.db();
    assert!(db.draft::<Pipeline>(self.id).expect("draft read").is_none());
    let versions = db.versions::<Pipeline>(self.id).expect("versions");
    assert_eq!(versions.len(), self.versions, "no unreviewed publication");
    assert_eq!(
      db.latest_version::<Pipeline>(self.id)
        .expect("latest")
        .expect("a version")
        .body(),
      &self.before,
      "immutable versions"
    );
  }

  /// Holds the widening in the panel and presses *Save*, which asks. Returns the pipeline it stores once allowed.
  fn proposed(&mut self) -> Pipeline {
    let key = self.before.steps()[0].key.clone();
    let mut edit = PipelineEdit::new(self.before.clone());
    match self.widens {
      Widens::Wait => {
        self
          .window
          .click_by_client("pipeline-hub-step-approval")
          .expect("approval proposal");
        edit.update(&key, |step| step.approval_gate = false);
      }
      Widens::Ask => {
        self
          .window
          .until("the ask switch", |s| s.shows("pipeline-hub-step-ask-tools"));
        self
          .window
          .click_by_client("pipeline-hub-step-ask-tools")
          .expect("ask proposal");
        edit.update(&key, |step| {
          if let StepKind::Agent { permissions, .. } = &mut step.kind {
            permissions.interactive_tool_approvals = false;
          }
        });
      }
    }
    self
      .window
      .until("the panel holds the change", |s| s.shows("pipeline-hub-save"));
    self.window.click_by_client("pipeline-hub-save").expect("Save asks");
    self
      .window
      .until("save question", |s| s.shows("pipeline-hub-confirm-confirm"));
    let asked = super::tools_window::text(&mut self.window, "pipeline-hub-confirm");
    let reason = match self.widens {
      Widens::Wait => "no longer wait for a person's approval",
      Widens::Ask => "no longer ask a person about its tool calls",
    };
    assert!(asked.contains(reason), "the question names what it widens: {asked}");
    edit.validate().expect("valid proposed pipeline")
  }
}

/// The hub's *Save* publishes at once, so one question covers what the old editor asked twice (Save, then Publish): a
/// refused or revoked *Allow* stores nothing, *Keep editing* needs only the permissions consent, and an *Allow* with it
/// publishes the next version and leaves no draft.
fn round_trip(f: &mut Fixture, expected: &Pipeline) {
  assert_eq!(
    f.window
      .click_by_client("pipeline-hub-confirm-confirm")
      .expect_err("off refuses")["code"],
    "person_only"
  );
  f.unchanged();
  f.window.let_automation_widen(true);
  f.window
    .click_by_client("pipeline-hub-confirm-keep")
    .expect("consented cancel");
  f.window
    .until("cancel unlocks the panel", |s| !s.shows("pipeline-hub-confirm-confirm"));
  f.unchanged();
  f.window
    .click_by_client("pipeline-hub-save")
    .expect("the held proposal asks again");
  f.window
    .until("the question again", |s| s.shows("pipeline-hub-confirm-confirm"));
  f.window.let_automation_widen(false);
  assert_eq!(
    f.window
      .click_by_client("pipeline-hub-confirm-confirm")
      .expect_err("revoked Allow refuses")["code"],
    "person_only"
  );
  f.unchanged();
  f.window.let_automation_widen(true);
  f.window
    .click_by_client("pipeline-hub-confirm-confirm")
    .expect("permissions consent permits Allow");
  let (db, id, next) = (f.db(), f.id, f.versions + 1);
  f.window.until("the next version", |_| {
    db.latest_version::<Pipeline>(id)
      .expect("latest")
      .is_some_and(|v| v.number().get() as usize == next)
  });
  f.window
    .until("question gone", |s| !s.shows("pipeline-hub-confirm-confirm"));
  let db = f.db();
  let versions = db.versions::<Pipeline>(f.id).expect("versions");
  assert_eq!(versions.len(), next);
  assert!(versions.iter().any(|v| v.body() == &f.before), "the old latest is kept");
  assert_eq!(
    db.latest_version::<Pipeline>(f.id)
      .expect("latest")
      .expect("new")
      .body(),
    expected
  );
  assert!(db.draft::<Pipeline>(f.id).expect("draft").is_none());
  assert!(
    !f.window
      .services()
      .workspace
      .automation_answers_approvals()
      .expect("approval consent remained off")
  );
}

#[test]
fn domain_pipeline_policy_wait_uses_permissions_only_for_cancel_save_and_publish() {
  let mut f = Fixture::new(Widens::Wait);
  let expected = f.proposed();
  round_trip(&mut f, &expected);
}

#[test]
fn domain_pipeline_policy_ask_uses_permissions_only_for_cancel_save_and_publish() {
  let mut f = Fixture::new(Widens::Ask);
  let expected = f.proposed();
  round_trip(&mut f, &expected);
}

fn queued_after_revoke(button: &str) {
  let mut f = Fixture::new(Widens::Wait);
  f.proposed();
  f.window.let_automation_widen(true);
  let found = f.window.ok("lurq_find_by_id", &json!({"id":button}));
  let reference = found
    .as_str()
    .expect("reference")
    .split_whitespace()
    .next()
    .expect("reference token")
    .to_owned();
  let (port, bearer, session) = (f.window.port, f.window.bearer(), f.window.session.clone());
  let body = json!({"jsonrpc":"2.0","id":95,"method":"tools/call","params":{"name":"lurq_interact","arguments":{"action":"click","ref":reference}}});
  let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
  let deadline = Instant::now() + Duration::from_secs(10);
  while !f.window.link.input_gate().input_in_flight() {
    assert!(!request.is_finished(), "request must actually enter queue");
    assert!(Instant::now() < deadline, "server forwarding deadline");
    thread::yield_now();
  }
  f.window.let_automation_widen(false);
  while !request.is_finished() {
    f.window.pass();
    thread::yield_now();
    assert!(Instant::now() < deadline, "delivery deadline");
  }
  request.join().expect("queued request completed");
  assert!(
    f.window.shows("pipeline-hub-confirm-confirm"),
    "revoked input cannot accept or cancel"
  );
  f.unchanged();
}

#[test]
fn domain_pipeline_policy_queued_save_cannot_execute_after_revocation() {
  queued_after_revoke("pipeline-hub-confirm-confirm");
}

#[test]
fn domain_pipeline_policy_queued_keep_cannot_execute_after_revocation() {
  queued_after_revoke("pipeline-hub-confirm-keep");
}
