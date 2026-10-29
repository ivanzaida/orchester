//! What auto mode's reviewer spends, end to end with `orchester-fake-claude` as the agent and the reviewer (and
//! `orchester-fake-codex` as a Codex reviewer): each review that asked a model is recorded with the request it judged,
//! its provider, the account its connection ran on, model, verdict and the tokens it reported; those tokens count
//! toward the run's budget and the project's daily budget like the agent's; the plan limits the reviewer's CLI reported
//! go to the history; a review whose usage cannot be read is recorded as unknown and counts nothing; a review the
//! person's answer stopped is recorded before the run ends; a reviewer whose CLI never started spent nothing and is not
//! recorded; and the answer that ends a reviewer's long output is still read. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test reviewer_usage reviewer_usage`.

mod support;

use orchester_core::{
  account::AccountCli,
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, IdempotencyKey, RunId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::{
    Answerer, ApprovalAnswer, PendingToolApproval, Run, TokenUsage,
    review::{AutoModeSettings, DEFAULT_REVIEW_MODEL, ReviewEnd, ReviewFailure, ReviewRecord, ReviewerChoice},
  },
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler_with, wait_for};

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

/// A run of `project`'s task started in auto mode with `reviewer` (the default one when `None`).
struct Started {
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(project: &TestProject, reviewer: Option<ReviewerChoice>) -> Self {
    let root = project.repo.path();
    let settings = AutoModeSettings { reviewer };
    ProjectDb::open(root)
      .expect("open")
      .save_auto_mode_settings(&settings)
      .expect("the reviewer");
    let (scheduler, events) = scheduler_with(root, project.vault.clone());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self { scheduler, events, id }
  }

  /// The first waiting request once `ready` says so of it in the store.
  async fn request(
    &mut self,
    root: &std::path::Path,
    ready: impl Fn(&ProjectDb, &PendingToolApproval) -> bool,
  ) -> PendingToolApproval {
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && ready(&db, request)
        {
          return request.clone();
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("a waiting request")
  }

  /// The person denies `request`, and the run ends.
  async fn deny(mut self, root: &std::path::Path, request: PendingToolApproval) -> Run {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
      .await
      .expect("denied");
    wait_for(root, &mut self.events, self.id, |run| run.status().is_terminal()).await
  }
}

/// A run of `project`'s task in auto mode with `reviewer` (the default one when `None`), once it ended.
async fn ended_run(project: &TestProject, reviewer: Option<ReviewerChoice>) -> (Run, ProjectDb) {
  let root = project.repo.path();
  let mut started = Started::new(project, reviewer).await;
  let run = wait_for(root, &mut started.events, started.id, |run| run.status().is_terminal()).await;
  (run, ProjectDb::open(root).expect("open"))
}

/// The agent's own counted tokens in `run`.
fn agent_tokens(db: &ProjectDb, run: RunId) -> TokensUsed {
  db.invocations(run)
    .expect("invocations")
    .iter()
    .map(|invocation| invocation.usage.counted_tokens())
    .sum()
}

/// The one review of `run`, recorded for the request it approved.
fn only_review(db: &ProjectDb, run: RunId) -> ReviewRecord {
  let reviews = db.reviewer_usage(run).expect("reviews");
  assert_eq!(reviews.len(), 1, "{reviews:?}");
  let decided = db.decided_tool_approvals(run).expect("decided");
  assert_eq!(
    reviews[0].approval, decided[0].request.id,
    "linked to the request it judged"
  );
  assert_eq!(reviews[0].invocation, decided[0].request.invocation);
  assert_eq!(reviews[0].execution, decided[0].request.execution);
  assert!(reviews[0].ended_at >= reviews[0].started_at);
  reviews[0].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_of_claude_code_counts_toward_the_run_and_the_day_and_keeps_its_limits() {
  let project = project(&claude("Run `cargo build`. Reviewer reports limits."));
  let (run, db) = ended_run(&project, None).await;
  let review = only_review(&db, run.id());
  assert_eq!(review.provider, AgentProvider::ClaudeCodeCli);
  assert_eq!(
    review.account,
    Some(AccountCli::ClaudeCode.default_account()),
    "the account its connection runs on"
  );
  assert_eq!(review.model.as_str(), DEFAULT_REVIEW_MODEL);
  let connection = review.connection.as_ref().expect("the connection the reviewer ran on");
  let connections = db.connections().expect("connections");
  let stored = connections
    .iter()
    .find(|stored| stored.id() == connection.id)
    .expect("the project's connection");
  assert_eq!(&*connection.name, stored.name().as_str(), "named as it was then");
  assert_eq!(review.end, ReviewEnd::Approved);
  let usage = TokenUsage {
    input: 200,
    cached_input: 50,
    output: 7,
  };
  assert_eq!(
    review.usage,
    Some(usage),
    "cache writes count as input, cache reads are cached"
  );
  let agent = agent_tokens(&db, run.id());
  assert!(agent.get() > 0, "the agent reported its own usage");
  let both = agent + TokensUsed::new(157);
  assert_eq!(run.used(), both, "the run's budget counts the reviewer");
  let today = db
    .tokens_used_since(Timestamp::from_unix_millis(0))
    .expect("the day's usage");
  assert_eq!(today, both, "the daily budget counts the reviewer");

  let history = db.limit_snapshots(Timestamp::from_unix_millis(0)).expect("history");
  assert_eq!(history.len(), 1, "{history:?}");
  assert_eq!(history[0].provider, AgentProvider::ClaudeCodeCli);
  assert_eq!(history[0].account, review.account, "its limits are the same account's");
  assert_eq!(history[0].limits.windows[0].percent(), Some(42.0));
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_that_cannot_be_read_is_unknown_and_counts_nothing() {
  let project = project(&claude("Run `cargo build`. Reviewer hides usage."));
  let (run, db) = ended_run(&project, None).await;
  let review = only_review(&db, run.id());
  assert_eq!(review.end, ReviewEnd::Approved);
  assert_eq!(review.usage, None, "unknown, never zero");
  let agent = agent_tokens(&db, run.id());
  assert_eq!(run.used(), agent);
  assert_eq!(
    db.tokens_used_since(Timestamp::from_unix_millis(0))
      .expect("the day's usage"),
    agent
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_of_codex_is_read_from_its_turn() {
  let project = project(&claude("Run `cargo build`."));
  let codex = support::agent_connection_of(AgentProvider::CodexCli);
  ProjectDb::open(project.repo.path())
    .expect("open")
    .create_connection(&codex)
    .expect("a Codex connection");
  let choice = ReviewerChoice {
    connection: codex.id(),
    model: ModelId::new("gpt-5.5-codex").expect("model"),
    effort: None,
  };
  let (run, db) = ended_run(&project, Some(choice)).await;
  let review = only_review(&db, run.id());
  assert_eq!(
    (review.provider, review.model.as_str()),
    (AgentProvider::CodexCli, "gpt-5.5-codex")
  );
  assert_eq!(review.account, Some(AccountCli::Codex.default_account()));
  let usage = TokenUsage {
    input: 1_200,
    cached_input: 1_000,
    output: 30,
  };
  assert_eq!(review.usage, Some(usage));
  assert_eq!(run.used(), agent_tokens(&db, run.id()) + TokensUsed::new(230));
  assert!(
    db.limit_snapshots(Timestamp::from_unix_millis(0))
      .expect("history")
      .is_empty(),
    "codex exec reports no plan limits"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_of_a_review_the_person_stopped_is_unknown_and_recorded_before_the_run_ends() {
  let project = project(&claude("Run `cargo build`. Reviewer hangs."));
  let root = project.repo.path();
  let mut started = Started::new(&project, None).await;
  let request = started
    .request(root, |db, request| {
      db.reviewing_tool_approvals()
        .expect("under review")
        .contains(&request.id)
    })
    .await;
  let run = started.deny(root, request).await;
  // Read as the run ended: the worker reports its end only once the stopped review has recorded its cost.
  let db = ProjectDb::open(root).expect("open");
  let reviews = db.reviewer_usage(run.id()).expect("reviews");
  assert_eq!(reviews.len(), 1, "{reviews:?}");
  assert_eq!(reviews[0].end, ReviewEnd::Failed(ReviewFailure::CouldNotRun));
  assert_eq!(reviews[0].usage, None, "a stopped reviewer spent an unknown amount");
  assert_eq!(run.used(), agent_tokens(&db, run.id()));
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_of_a_reviewer_that_never_started_is_not_recorded() {
  let project = project(&claude("Run `cargo build`."));
  let root = project.repo.path();
  let missing = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · missing").expect("name"),
    AgentProvider::ClaudeCodeCli,
    root.join("no-such-claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute path")
  .into_connection();
  ProjectDb::open(root)
    .expect("open")
    .create_connection(&missing)
    .expect("a connection to a CLI that is not there");
  let choice = ReviewerChoice {
    connection: missing.id(),
    model: ModelId::new(DEFAULT_REVIEW_MODEL).expect("model"),
    effort: None,
  };
  let mut started = Started::new(&project, Some(choice)).await;
  let request = started
    .request(root, |db, request| {
      db.tool_approval_review(request.run, &request.id)
        .expect("review")
        .is_some()
    })
    .await;
  let run = started.deny(root, request).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.reviewer_usage(run.id()).expect("reviews").is_empty(),
    "a CLI that never started asked no model"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_usage_is_read_after_more_output_than_the_runner_keeps() {
  let project = project(&claude("Run `cargo build`. Reviewer rambles."));
  let (run, db) = ended_run(&project, None).await;
  let review = only_review(&db, run.id());
  assert_eq!(review.end, ReviewEnd::Approved, "the verdict ends the output");
  let usage = TokenUsage {
    input: 200,
    cached_input: 50,
    output: 7,
  };
  assert_eq!(review.usage, Some(usage));
}
