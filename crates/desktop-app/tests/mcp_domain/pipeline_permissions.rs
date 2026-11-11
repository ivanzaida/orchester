//! Pipeline setup gates use permission consent, independently of runtime approval answering. A local project's
//! pipeline is edited in the Pipelines hub's panel, whose *Save* publishes the next version at once and asks *Allow*
//! for what it widens: a step that stops waiting for a person's approval, or stops asking about its agent's tool calls.
//! The panel turns the first off on the step; for the second it has no control yet, so the test restores a version that
//! does not ask over one that does.
use std::{
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use orchester_app::routes::{PipelineView, PipelinesFilter, PipelinesView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  resource::{Draft, Pipeline, PipelineEdit, ResourceKind, StepKind},
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
  /// The first step stops asking about its agent's tool calls: *Restore* of v2 over v3.
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

/// Publishes `body` as the pipeline's next version, as another editor would behind the app.
fn publish(db: &mut ProjectDb, id: ResourceId, body: Pipeline) {
  let latest = db
    .latest_version::<Pipeline>(id)
    .expect("read version")
    .expect("published fixture");
  db.publish_pipeline_edit(id, &body, latest.number(), None, Timestamp::now())
    .expect("next version");
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
    let mut db = ProjectDb::open(&seeded.repo).expect("private database");
    let id = db
      .resources(ResourceKind::Pipeline)
      .expect("resources")
      .into_iter()
      .find(|row| row.name().as_str() == "Narrowed permissions")
      .expect("narrowed fixture")
      .id();
    let latest = |db: &ProjectDb| {
      db.latest_version::<Pipeline>(id)
        .expect("read version")
        .expect("published fixture")
        .body()
        .clone()
    };
    if widens == Widens::Ask {
      // v1 asks. v2 does not, and v3 asks again (and says so in its description, as a body must differ to be a
      // version), so that restoring v2 over v3 is the widening.
      let key = latest(&db).steps()[0].key.clone();
      let mut edit = PipelineEdit::new(latest(&db));
      edit.update(&key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          permissions.interactive_tool_approvals = false;
        }
      });
      publish(&mut db, id, edit.validate().expect("v2"));
      let asking = latest(&db).with_description("asks again".into());
      let mut edit = PipelineEdit::new(asking);
      edit.update(&key, |step| {
        if let StepKind::Agent { permissions, .. } = &mut step.kind {
          permissions.interactive_tool_approvals = true;
        }
      });
      publish(&mut db, id, edit.validate().expect("v3"));
    }
    let before = latest(&db);
    let versions = db.versions::<Pipeline>(id).expect("versions").len();
    drop(db);
    let runner = folder.path().join("no-runner/orchester-runner");
    let mut window = Served::start_with(folder, &seeded.app_database, runner, true);
    window.until("navigation attached", |s| s.link.navigation().is_ok());
    let page = match widens {
      Widens::Wait => ProjectPage::Pipeline(id, PipelineView::Step(before.steps()[0].key.clone())),
      Widens::Ask => ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(id))),
    };
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

  /// Version `number` of the pipeline as stored.
  fn stored(&self, number: u32) -> Pipeline {
    self
      .db()
      .versions::<Pipeline>(self.id)
      .expect("versions")
      .into_iter()
      .find(|version| version.number().get() == number)
      .expect("the version")
      .body()
      .clone()
  }

  /// Holds the widening in the panel and presses *Save*, which asks. Returns the pipeline it stores once allowed.
  fn proposed(&mut self) -> Pipeline {
    let key = self.before.steps()[0].key.clone();
    let edit = match self.widens {
      Widens::Wait => {
        let mut edit = PipelineEdit::new(self.before.clone());
        self
          .window
          .click_by_client("pipeline-hub-step-approval")
          .expect("approval proposal");
        edit.update(&key, |step| step.approval_gate = false);
        edit
      }
      Widens::Ask => {
        for id in [
          "pipeline-hub-versions-button",
          "pipeline-hub-version-2",
          "pipeline-hub-version-restore",
        ] {
          self.window.until(id, |s| s.shows(id));
          self.window.click_by_client(id).expect("restore v2");
        }
        // Restoring stores v2 as it was.
        PipelineEdit::new(self.stored(2))
      }
    };
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
