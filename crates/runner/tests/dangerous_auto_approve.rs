//! The project's *Dangerous auto-approve mode* end to end (`docs/design/approval-modes.md`, the person's decision of
//! 2026-11-11), with `orchester-fake-claude` as the agent and the reviewer: while it is on, every tool request is
//! approved at once whatever would hold it — a push, a destructive command, a credential read, a write outside the
//! folders, a command Orchester cannot read, a plain one — in Manual and in Autopilot, with no reviewer started and no
//! card, recorded as `dangerous_auto_approve`; a request already waiting for the person when it is turned on is
//! approved then. The fake only records that it ran a command; none is executed. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test dangerous_auto_approve dangerous_auto_approve`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, ApprovalMode, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, VariableName},
  run::{AnswerKind, AnswerRecord, Answerer, ApprovalAnswer, ApproveMode, DecidedBy, PersonReason, RequestAnswer},
  store::{ActivityStore, SettingsStore},
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reviewer::with_default_reviewer, scheduler_with, wait_for};

/// Commands each kind of hard hold keeps from the reviewer, and a plain one.
const ASKED: [&str; 6] = [
  "Run `git push origin HEAD`.",
  "Run `rm -rf ../../outside`.",
  "Run `cat ~/.ssh/id_rsa`.",
  "Run `cp a.txt ../../b.txt`.",
  "Run ``.",
  "Run `cargo build`.",
];

/// A Claude Code Builder in `mode` that asks before every command, in a project with the default reviewer.
fn asking(brief: &'static str, mode: ApprovalMode) -> TestProject {
  let mut permissions = AgentPermissions::new(RepositoryAccess::Write, CommandPermission::AskBefore, NetworkAccess::On);
  permissions.set_approval_mode(mode);
  with_default_reviewer(project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    ..Setup::new(brief)
  }))
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
  /// Where the fake reviewer writes what it was asked.
  log: TempDir,
}

impl Started {
  /// A run of `project`'s task in a project whose Approve mode is `mode`.
  async fn new(project: TestProject, mode: ApproveMode) -> Self {
    let log = tempfile::tempdir().expect("a folder for the review log");
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    db.save_approve_mode(mode).expect("the approve mode");
    let variable = VariableName::new("FAKE_REVIEW_LOG").expect("name");
    let path = log.path().join("review.log").display().to_string();
    let environment = CommandEnvironment::new(BTreeMap::from([(variable, path)])).expect("environment");
    db.save_project_environment(&environment).expect("project environment");
    let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self {
      project,
      scheduler,
      events,
      id,
      log,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  async fn ended(&mut self) {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await;
  }

  /// Waits until the run's first request came to the person; why it came.
  async fn came_to_person(&mut self) -> PersonReason {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(came) = db.came_to_person(id, &request.id).expect("reason")
        {
          return came.reason;
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("a request comes to the person")
  }

  fn answers(&self) -> Vec<AnswerRecord> {
    self.db().request_answers(self.id).expect("answers")
  }

  /// How each tool request of the run was decided, from its activity.
  fn decisions(&self) -> Vec<DecidedBy> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => Some(decision.decided_by),
        _ => None,
      })
      .collect()
  }

  /// Whether the fake reviewer was started at all.
  fn reviewed(&self) -> bool {
    std::fs::read_to_string(self.log.path().join("review.log")).is_ok_and(|log| log.contains("pid "))
  }
}

fn dangerous() -> RequestAnswer {
  RequestAnswer::by(AnswerKind::DangerousAutoApprove)
}

#[tokio::test(flavor = "multi_thread")]
async fn dangerous_auto_approve_approves_every_request_at_once_in_both_modes() {
  for mode in [ApprovalMode::Manual, ApprovalMode::Autopilot] {
    for brief in ASKED {
      let mut started = Started::new(asking(brief, mode), ApproveMode::DangerousAutoApprove).await;
      started.ended().await;
      let answers = started.answers();
      assert_eq!(answers.len(), 1, "{mode:?} {brief}: {answers:?}");
      assert_eq!(answers[0].answer.as_ref(), Some(&dangerous()), "{mode:?} {brief}");
      assert_eq!(
        answers[0].came_to_person, None,
        "{mode:?} {brief}: no card, nobody asked"
      );
      assert_eq!(started.decisions(), [DecidedBy::AutoApproved], "{mode:?} {brief}");
      assert!(!started.reviewed(), "{mode:?} {brief}: no reviewer started");
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn dangerous_auto_approve_approves_a_request_waiting_when_it_is_turned_on() {
  let mut started = Started::new(
    asking("Run `git push origin HEAD`.", ApprovalMode::Manual),
    ApproveMode::Autopilot,
  )
  .await;
  assert_eq!(started.came_to_person().await, PersonReason::Push);
  started
    .db()
    .save_approve_mode(ApproveMode::DangerousAutoApprove)
    .expect("turned on");
  started.ended().await;
  let answers = started.answers();
  assert_eq!(answers[0].answer.as_ref(), Some(&dangerous()));
  assert_eq!(
    answers[0].came_to_person.as_ref().map(|came| came.reason),
    Some(PersonReason::Push),
    "it had come to the person before"
  );
  assert!(
    started
      .db()
      .pending_tool_approvals(started.id)
      .expect("requests")
      .is_empty()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn dangerous_auto_approve_off_leaves_the_hold_rules_in_place() {
  let mut started = Started::new(
    asking("Run `git push origin HEAD`.", ApprovalMode::Manual),
    ApproveMode::Autopilot,
  )
  .await;
  assert_eq!(started.came_to_person().await, PersonReason::Push);
  assert!(!started.reviewed(), "a hold asks the person without the reviewer");
  let request = started.db().pending_tool_approvals(started.id).expect("requests")[0].clone();
  started
    .scheduler
    .decide_tool_approval(started.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  started.ended().await;
  assert_eq!(
    started.answers()[0].answer.as_ref(),
    Some(&RequestAnswer::by(AnswerKind::Person))
  );
}
