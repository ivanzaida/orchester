//! Pipeline setup gates use permission consent, independently of runtime approval answering.
use std::{
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{ProjectPage, Route};
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

struct Fixture {
  window: Served,
  repo: PathBuf,
  id: ResourceId,
  before: Pipeline,
}

impl Fixture {
  fn new() -> Self {
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
    let before = db
      .latest_version::<Pipeline>(id)
      .expect("read version")
      .expect("published fixture")
      .body()
      .clone();
    let runner = folder.path().join("no-runner/orchester-runner");
    let mut window = Served::start_with(folder, &seeded.app_database, runner, true);
    window.until("navigation attached", |s| s.link.navigation().is_ok());
    window.link.navigation().expect("navigation").go(Route::Project(
      seeded.project,
      ProjectPage::Pipeline(id, Default::default()),
    ));
    window.until("pipeline editor", |s| s.shows("pipeline-add-step"));
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
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("private database")
  }

  fn unchanged(&self) {
    let db = self.db();
    let versions = db.versions::<Pipeline>(self.id).expect("versions");
    assert_eq!(versions.len(), 1, "no unreviewed publication");
    assert_eq!(versions[0].body(), &self.before, "immutable v1");
  }

  fn proposed(&mut self, toggle: &str) -> Pipeline {
    let key = self.before.steps()[0].key.clone();
    self
      .window
      .click_by_client(&format!("pipeline-step-{key}"))
      .expect("step selected");
    if toggle == "step-ask-tools" {
      // Asking about tool calls is set on the step's Permissions page.
      self
        .window
        .click_by_client("step-open-permissions")
        .expect("the step's permissions");
    }
    self.window.click_by_client(toggle).expect("permission proposal");
    self
      .window
      .click_by_client("pipeline-save")
      .expect("Save requests review");
    self
      .window
      .until("save question", |s| s.shows("pipeline-save-question-confirm"));
    let mut edit = PipelineEdit::new(self.before.clone());
    edit.update(&key, |step| {
      if toggle == "step-gate" {
        step.approval_gate = false;
      } else if let StepKind::Agent { permissions, .. } = &mut step.kind {
        permissions.interactive_tool_approvals = false;
      }
    });
    edit.validate().expect("valid proposed pipeline")
  }
}

fn save_round_trip(f: &mut Fixture, expected: &Pipeline) {
  assert_eq!(
    f.window
      .click_by_client("pipeline-save-question-confirm")
      .expect_err("off refuses")["code"],
    "person_only"
  );
  assert!(f.db().draft::<Pipeline>(f.id).expect("draft read").is_none());
  f.unchanged();
  f.window.let_automation_widen(true);
  f.window
    .click_by_client("pipeline-save-question-keep")
    .expect("consented cancel");
  f.window
    .until("cancel unlocks editor", |s| !s.shows("pipeline-save-question-confirm"));
  assert!(f.db().draft::<Pipeline>(f.id).expect("draft read").is_none());
  f.window
    .click_by_client("pipeline-save")
    .expect("same proposal requests review");
  f.window
    .until("save question again", |s| s.shows("pipeline-save-question-confirm"));
  f.window
    .click_by_client("pipeline-save-question-confirm")
    .expect("permissions consent permits Save");
  let (db, id) = (f.db(), f.id);
  f.window.until("saved exact candidate", |_| {
    db.draft::<Pipeline>(id)
      .expect("draft")
      .is_some_and(|d| d.body() == expected)
  });
  f.window.until("save question dismissed", |s| {
    !s.shows("pipeline-save-question-confirm")
  });
  f.unchanged();
}

fn publish_round_trip(f: &mut Fixture, expected: &Pipeline) {
  f.window
    .click_by_client("pipeline-publish")
    .expect("publication review");
  f.window
    .until("publication question", |s| s.shows("pipeline-publish-question-confirm"));
  f.window
    .click_by_client("pipeline-publish-question-keep")
    .expect("consented publication cancel");
  f.window.until("publication cancel unlocks", |s| {
    !s.shows("pipeline-publish-question-confirm")
  });
  assert_eq!(
    f.db()
      .draft::<Pipeline>(f.id)
      .expect("draft")
      .expect("saved draft")
      .body(),
    expected
  );
  f.unchanged();
  f.window
    .click_by_client("pipeline-publish")
    .expect("publication review again");
  f.window.until("publication question again", |s| {
    s.shows("pipeline-publish-question-confirm")
  });
  f.window.let_automation_widen(false);
  assert_eq!(
    f.window
      .click_by_client("pipeline-publish-question-confirm")
      .expect_err("revoked Publish refuses")["code"],
    "person_only"
  );
  f.unchanged();
  f.window.let_automation_widen(true);
  f.window
    .click_by_client("pipeline-publish-question-confirm")
    .expect("permissions-only Publish");
  let (db, id) = (f.db(), f.id);
  f.window.until("actual v2", |_| {
    db.latest_version::<Pipeline>(id)
      .expect("latest")
      .is_some_and(|v| v.number().get() == 2)
  });
  f.window.until("published question gone", |s| {
    !s.shows("pipeline-publish-question-confirm")
  });
  let db = f.db();
  let versions = db.versions::<Pipeline>(f.id).expect("versions");
  assert_eq!(versions.len(), 2);
  assert!(versions.iter().any(|v| v.number().get() == 1 && v.body() == &f.before));
  assert_eq!(
    db.latest_version::<Pipeline>(f.id).expect("latest").expect("v2").body(),
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
  let mut f = Fixture::new();
  let expected = f.proposed("step-gate");
  save_round_trip(&mut f, &expected);
  publish_round_trip(&mut f, &expected);
}

#[test]
fn domain_pipeline_policy_ask_uses_permissions_only_for_cancel_save_and_publish() {
  let mut f = Fixture::new();
  let expected = f.proposed("step-ask-tools");
  save_round_trip(&mut f, &expected);
  publish_round_trip(&mut f, &expected);
}

fn queued_after_revoke(button: &str) {
  let mut f = Fixture::new();
  f.proposed("step-gate");
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
    f.window.shows("pipeline-save-question-confirm"),
    "revoked input cannot accept or cancel"
  );
  assert!(f.db().draft::<Pipeline>(f.id).expect("draft").is_none());
  f.unchanged();
}

#[test]
fn domain_pipeline_policy_queued_save_cannot_execute_after_revocation() {
  queued_after_revoke("pipeline-save-question-confirm");
}

#[test]
fn domain_pipeline_policy_queued_keep_cannot_execute_after_revocation() {
  queued_after_revoke("pipeline-save-question-keep");
}
