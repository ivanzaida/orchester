//! A run of `orchester-fake-codex` whose code-mode cell yields an Orchester call (`fake_codex_cell`), shared by the
//! tool-wait tests.

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::stream::AgentEvent,
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{Answerer, ApprovalAnswer, PendingToolApproval},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use super::{PATIENCE, Setup, TestProject, project, reported, scheduler_with};

/// A run whose yielded cell's outside search waits for the person (`fake_codex_cell`).
pub struct Waiting {
  pub project: TestProject,
  pub runner: Scheduler,
  pub events: mpsc::UnboundedReceiver<Event>,
  pub id: RunId,
  pub request: PendingToolApproval,
}

impl Waiting {
  pub async fn start() -> Self {
    Self::start_with("Yield outside read.").await
  }

  pub async fn start_with(brief: &'static str) -> Self {
    Self::begin(&Setup::new(brief)).await
  }

  pub async fn begin(setup: &Setup) -> Self {
    let project = project(setup);
    let root = project.repo.path();
    let folder = root.join("deps-a");
    std::fs::create_dir(&folder).expect("outside folder");
    std::fs::write(folder.join("notes.txt"), "outside line").expect("outside file");
    let (runner, mut events) = scheduler_with(root, project.vault.clone());
    let id = runner
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("start");
    let request = pending(root, &mut events, id, |_| true).await;
    Self {
      project,
      runner,
      events,
      id,
      request,
    }
  }
}

/// The run's first pending approval that is `wanted`.
pub async fn pending(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  id: RunId,
  wanted: impl Fn(&PendingToolApproval) -> bool,
) -> PendingToolApproval {
  tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(request) = ProjectDb::open(root)
        .expect("open")
        .pending_tool_approvals(id)
        .expect("pending")
        .into_iter()
        .find(|request| wanted(request))
      {
        return request;
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("approval arrives")
}

/// Lets the fake's model end its turn and waits until it has.
pub async fn ended_turn(root: &Path, id: RunId) {
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  std::fs::write(worktree.join("yield-turn.txt"), "end the model turn").expect("release turn");
  tokio::time::timeout(PATIENCE, async {
    loop {
      if ProjectDb::open(root)
        .expect("open")
        .activity(id, 0)
        .expect("activity")
        .iter()
        .any(|entry| matches!(entry.event, AgentEvent::TurnCompleted { .. }))
      {
        return;
      }
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .expect("the model ends the turn");
}

/// `brief` in a step whose commands outside the sandbox ask a person, so a cell's command reaches one.
pub fn command_setup(brief: &'static str) -> Setup {
  Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new(brief)
  }
}

impl Waiting {
  /// Approves the cell's search before its turn ends and waits until the relay has returned it.
  pub async fn approve_in_turn(&mut self) {
    let request = self.request.id.clone();
    self
      .runner
      .decide_tool_approval(self.id, request, ApprovalAnswer::Once, Answerer::Person)
      .await
      .expect("decide the call");
    let db = ProjectDb::open(self.project.repo.path()).expect("open");
    tokio::time::timeout(PATIENCE, async {
      while reported(&db, self.id, "cell-returned: ").is_empty() {
        self.events.recv().await.expect("notification");
      }
    })
    .await
    .expect("the relay returns the call");
  }
}
