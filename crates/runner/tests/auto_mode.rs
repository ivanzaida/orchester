//! Auto mode end to end with `orchester-fake-claude` as both the agent and the reviewer: a command the
//! rules leave to a person is approved by the reviewer without the person, in an empty folder of its own
//! with its system prompt; a credential read, an unreadable script and a Codex escalation still ask the
//! person without a review; a question of the reviewer and an answer it cannot read leave the request to
//! the person with what auto mode said; a review whose run turned auto mode off meanwhile is dropped; and
//! a reviewer still at work when the person answers is stopped. A Codex connection reviews through `codex
//! exec` in its read-only sandbox (`orchester-fake-codex` as the reviewer). Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test auto_mode auto_mode`.

mod support;

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, ModelId, VariableName},
  run::{
    Answerer, ApprovalAnswer, DecidedBy, PendingToolApproval, Run,
    review::{AutoModeSettings, Review, ReviewFailure, ReviewHold, ReviewerChoice},
  },
  store::{ActivityStore, ConnectionStore, SettingsStore},
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, is_running, pending_approval, project, scheduler_with, wait_for};

/// A Claude Code Builder that runs `git` and asks about every other command.
fn claude(brief: &'static str) -> Setup {
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(listed),
      NetworkAccess::On,
    )),
    ..Setup::new(brief)
  }
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
  /// Where the fake reviewer writes where it ran.
  log: TempDir,
}

impl Started {
  /// A run of `setup` in a project whose new runs use auto mode, with the default reviewer.
  async fn new(setup: &Setup) -> Self {
    Self::of(project(setup), None).await
  }

  /// A run of `project`'s task, its new runs using auto mode with `reviewer`.
  async fn of(project: TestProject, reviewer: Option<ReviewerChoice>) -> Self {
    let log = tempfile::tempdir().expect("a folder for the review log");
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    let settings = AutoModeSettings {
      new_runs: true,
      reviewer,
    };
    db.save_auto_mode_settings(&settings).expect("auto mode for new runs");
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

  async fn pending(&mut self) -> PendingToolApproval {
    pending_approval(self.project.repo.path(), &mut self.events, self.id).await
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

  /// What the fake reviewer wrote about where it ran: `key value` lines.
  fn logged(&self, key: &str) -> Vec<String> {
    std::fs::read_to_string(self.log.path().join("review.log"))
      .unwrap_or_default()
      .lines()
      .filter_map(|line| line.strip_prefix(&format!("{key} ")).map(str::to_owned))
      .collect()
  }

  /// Waits until the fake reviewer has written about where it runs.
  async fn reviewer_started(&self) -> String {
    tokio::time::timeout(PATIENCE, async {
      loop {
        if let Some(pid) = self.logged("pid").first() {
          return pid.clone();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
      }
    })
    .await
    .expect("the reviewer starts")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_approves_a_command_that_serves_the_task_in_a_folder_of_its_own() {
  let mut started = Started::new(&claude("Run `cargo build`.")).await;
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
  // It ran in an empty folder outside the project, removed after the review, with its system prompt.
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  assert!(!cwd.starts_with(started.project.repo.path()), "{}", cwd.display());
  assert_eq!(started.logged("empty"), ["true"]);
  assert_eq!(started.logged("instructions"), ["true"]);
  assert!(!cwd.exists(), "the review's folder is removed");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_leaves_a_credential_read_and_a_script_to_the_person_without_a_review() {
  for (brief, hold) in [
    ("Run `cp ~/.orchester-test-token token.txt`.", ReviewHold::Secrets),
    ("Run `./regen.sh`.", ReviewHold::Unreadable),
  ] {
    let mut started = Started::new(&claude(brief)).await;
    let (request, review) = started.left_to_person().await;
    assert_eq!(review, Review::Held { hold }, "{brief}");
    started.deny(request).await;
    started.ended().await;
    let decisions = started.decisions();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].0, DecidedBy::PersonDeny);
    assert_eq!(decisions[0].1, Some(review));
    let counts = started.db().decision_counts(started.id).expect("counts");
    assert_eq!((counts.by_reviewer, counts.asked_by_reviewer), (0, 1));
    assert!(started.logged("pid").is_empty(), "{brief}: no reviewer ran");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_leaves_a_codex_escalation_to_the_person() {
  let setup = Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::Off,
    )),
    ..Setup::new("Escalate `cargo build`.")
  };
  let mut started = Started::new(&setup).await;
  let (request, review) = started.left_to_person().await;
  assert!(request.command);
  assert_eq!(
    review,
    Review::Held {
      hold: ReviewHold::Escalation
    }
  );
  started.deny(request).await;
  started.ended().await;
  assert!(started.logged("pid").is_empty(), "no reviewer ran");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_asks_the_person_when_the_reviewer_asks_or_babbles() {
  let mut asked = Started::new(&claude("Run `cargo build`. Reviewer asks.")).await;
  let (request, review) = asked.left_to_person().await;
  assert!(
    matches!(&review, Review::Asked { reason, model } if reason == "The fake reviewer leaves it to the person."
      && model == "claude-haiku-4-5-20251001"),
    "{review:?}"
  );
  asked.deny(request).await;
  asked.ended().await;

  let mut babbled = Started::new(&claude("Run `cargo build`. Reviewer babbles.")).await;
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

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_turned_off_during_a_review_drops_its_approval() {
  let mut started = Started::new(&claude("Run `cargo build`. Reviewer waits.")).await;
  let request = started.pending().await;
  started.reviewer_started().await;
  started
    .db()
    .set_run_auto_mode(started.id, false)
    .expect("a person turns auto mode off");
  // The reviewer approves after three seconds; its approval must not answer the request.
  tokio::time::sleep(Duration::from_secs(6)).await;
  let db = started.db();
  assert_eq!(db.pending_tool_approvals(started.id).expect("requests").len(), 1);
  assert_eq!(db.tool_approval_review(started.id, &request.id).expect("review"), None);
  started.deny(request).await;
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions, [(DecidedBy::PersonDeny, None)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_stops_the_reviewer_when_the_person_answers_first() {
  let mut started = Started::new(&claude("Run `cargo build`. Reviewer hangs.")).await;
  let request = started.pending().await;
  let pid = started.reviewer_started().await;
  assert!(is_running(&pid));
  started.deny(request).await;
  started.ended().await;
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  tokio::time::timeout(PATIENCE, async {
    while is_running(&pid) || cwd.exists() {
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  })
  .await
  .expect("the reviewer is stopped and its folder removed");
  assert_eq!(started.decisions(), [(DecidedBy::PersonDeny, None)]);
}

/// A run of a Claude Code Builder reviewed by a Codex connection of the project, with `model` and effort.
async fn reviewed_by_codex(brief: &'static str) -> Started {
  let setup = claude(brief);
  let project = project(&setup);
  let codex = support::agent_connection_of(AgentProvider::CodexCli);
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.create_connection(&codex).expect("a Codex connection");
  let choice = ReviewerChoice {
    connection: codex.id(),
    model: ModelId::new("gpt-5.5-codex").expect("model"),
    effort: Some("low".to_owned()),
  };
  Started::of(project, Some(choice)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_reviews_with_a_codex_connection_in_its_read_only_sandbox() {
  let mut started = reviewed_by_codex("Run `cargo build`.").await;
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert!(
    matches!(&decisions[0].1, Some(Review::Approved { reason, model })
      if reason == "The fake Codex reviewer finds it serves the task." && model == "gpt-5.5-codex"),
    "{decisions:?}"
  );
  let argv = started.logged("argv").remove(0);
  for part in [
    "-a never exec --json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check --sandbox read-only",
    "-m gpt-5.5-codex -c model_reasoning_effort=low",
    "-c features.shell_tool=false -c tools.web_search=false --output-schema ",
  ] {
    assert!(argv.contains(part), "{part} in {argv}");
  }
  assert!(argv.ends_with(" -"), "the message comes on standard input: {argv}");
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  assert!(!cwd.starts_with(started.project.repo.path()), "{}", cwd.display());
  assert_eq!(started.logged("empty"), ["true"]);
  assert_eq!(started.logged("schema"), ["true"]);
  assert_eq!(started.logged("instructions"), ["true"]);
  assert!(!cwd.exists(), "the review's folder is removed");

  let mut failed = reviewed_by_codex("Run `cargo build`. Reviewer fails.").await;
  let (request, review) = failed.left_to_person().await;
  assert!(
    matches!(&review, Review::Failed { failure: ReviewFailure::CouldNotRun, model: Some(model) } if model == "gpt-5.5-codex"),
    "{review:?}"
  );
  failed.deny(request).await;
  failed.ended().await;
}
