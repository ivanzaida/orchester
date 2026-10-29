//! Approval modes end to end (`docs/design/approval-modes.md`) with `orchester-fake-claude` as the agent and the
//! reviewer. Agent profiles have no approval mode yet, so every run is in Manual by its profile, whether or not its
//! project allows Autopilot: a worktree command goes to the reviewer, and a network command the reviewer asks about
//! waits for the person. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test approval_mode_effective approval_mode_effective`.

mod support;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{AllowAutopilot, Answerer, ApprovalAnswer, DecidedBy, EffectiveMode, ManualBecause, Run, review::Review},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler_with, wait_for, with_default_reviewer};

/// A Claude Code Builder that asks before every command, in a project with the default reviewer.
fn asking(brief: &'static str) -> TestProject {
  with_default_reviewer(project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      NetworkAccess::On,
    )),
    ..Setup::new(brief)
  }))
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  /// A run of `project`'s task, in a project that allows Autopilot as `allow` says, with the default reviewer.
  async fn new(project: TestProject, allow: AllowAutopilot) -> Self {
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    db.save_allow_autopilot(allow).expect("allow autopilot");
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
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  /// Who answered the run's requests, as the activity records it.
  fn decided(&self) -> Vec<DecidedBy> {
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

  /// The first waiting request once the reviewer left it to the person, with what the reviewer did.
  async fn left_to_person(&mut self) -> (String, Review) {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
        {
          return (request.id.clone(), review);
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("the reviewer leaves a request to the person")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_effective_manual_reviews_a_worktree_command_whether_or_not_autopilot_is_allowed() {
  for allow in [AllowAutopilot::OFF, AllowAutopilot::ON] {
    let mut started = Started::new(asking("Run `cargo test -p core`."), allow).await;
    assert_eq!(
      started.db().run_approval_mode(started.id).expect("mode"),
      EffectiveMode::Manual(ManualBecause::Profile),
      "{allow:?}"
    );
    started.ended().await;
    assert_eq!(started.decided(), [DecidedBy::Reviewer], "{allow:?}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn approval_mode_effective_manual_leaves_a_network_command_the_reviewer_asks_about_to_the_person() {
  let mut started = Started::new(
    asking("Run `git fetch origin`. Reviewer asks."),
    AllowAutopilot::ON,
  )
  .await;
  let (request, review) = started.left_to_person().await;
  assert!(matches!(review, Review::Asked { .. }), "{review:?}");
  started
    .scheduler
    .decide_tool_approval(started.id, request, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  started.ended().await;
  assert_eq!(started.decided(), [DecidedBy::PersonDeny]);
}
