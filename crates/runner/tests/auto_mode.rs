//! Auto mode end to end with `orchester-fake-claude` as both the agent and the reviewer: a command the
//! rules leave to a person (an unreadable script) is approved by the reviewer without the person, a
//! credential read still asks the person without a review, a question of the reviewer and an answer it
//! cannot read leave the request to the person with what auto mode said. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test auto_mode auto_mode`.

mod support;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  run::{
    Answerer, ApprovalAnswer, DecidedBy, PendingToolApproval, Run,
    review::{AutoModeSettings, Review, ReviewFailure, ReviewHold},
  },
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler_with, wait_for};

/// A Claude Code Builder that runs `git` and asks about every other command, in a project whose new runs
/// use auto mode.
fn builder(brief: &'static str) -> TestProject {
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(listed),
    NetworkAccess::On,
  );
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(permissions),
    ..Setup::new(brief)
  });
  let settings = AutoModeSettings {
    new_runs: true,
    ..AutoModeSettings::default()
  };
  ProjectDb::open(project.repo.path())
    .expect("open")
    .save_auto_mode_settings(&settings)
    .expect("auto mode for new runs");
  project
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(brief: &'static str) -> Self {
    let project = builder(brief);
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

  /// The first waiting request, once auto mode has left it to the person.
  async fn left_to_person(&mut self) -> (PendingToolApproval, Review) {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
        {
          return (request.clone(), review);
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("auto mode leaves a request to the person")
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  /// How the run's requests were answered, with what auto mode did.
  fn decisions(&self) -> Vec<(DecidedBy, Option<Review>)> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => Some((decision.decided_by, decision.review)),
        _ => None,
      })
      .collect()
  }

  async fn deny(&mut self, request: PendingToolApproval) {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
      .await
      .expect("denied");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_approves_an_unreadable_script_that_serves_the_task() {
  let mut started = Started::new("Run `./regen.sh`.").await;
  assert!(started.db().run_auto_mode(started.id).expect("read"));
  let run = started.ended().await;
  assert!(run.status().is_terminal());
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert!(
    matches!(&decisions[0].1, Some(Review::Approved { reason, .. }) if reason == "The fake reviewer finds it serves the task."),
    "{decisions:?}"
  );
  let counts = started.db().decision_counts(started.id).expect("counts");
  assert_eq!((counts.by_reviewer, counts.asked_by_reviewer, counts.by_you), (1, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_leaves_a_credential_read_to_the_person_without_a_review() {
  let mut started = Started::new("Run `cp ~/.orchester-test-token token.txt`.").await;
  let (request, review) = started.left_to_person().await;
  assert_eq!(
    review,
    Review::Held {
      hold: ReviewHold::Secrets
    }
  );
  started.deny(request).await;
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1);
  assert_eq!(decisions[0].0, DecidedBy::PersonDeny);
  assert_eq!(decisions[0].1, Some(review));
  let counts = started.db().decision_counts(started.id).expect("counts");
  assert_eq!((counts.by_reviewer, counts.asked_by_reviewer), (0, 1));
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_asks_the_person_when_the_reviewer_asks_or_babbles() {
  let mut asked = Started::new("Run `./regen.sh`. Reviewer asks.").await;
  let (request, review) = asked.left_to_person().await;
  assert!(
    matches!(&review, Review::Asked { reason, model } if reason == "The fake reviewer leaves it to the person."
      && model == "claude-haiku-4-5-20251001"),
    "{review:?}"
  );
  asked.deny(request).await;
  asked.ended().await;

  let mut babbled = Started::new("Run `./regen.sh`. Reviewer babbles.").await;
  let (request, review) = babbled.left_to_person().await;
  assert!(
    matches!(
      review,
      Review::Failed {
        failure: ReviewFailure::Unreadable,
        ..
      }
    ),
    "{review:?}"
  );
  babbled.deny(request).await;
  babbled.ended().await;
}
