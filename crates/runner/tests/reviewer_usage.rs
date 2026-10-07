//! What auto mode's reviewer spends, end to end with `orchester-fake-claude` as the agent and the reviewer (and
//! `orchester-fake-codex` as a Codex reviewer): each review that asked a model is recorded with the request it
//! judged, its provider, model, verdict and the tokens it reported; those tokens count toward the run's budget and
//! the project's daily budget like the agent's; the plan limits the reviewer's CLI reported go to the history; a
//! review whose usage cannot be read is recorded as unknown and counts nothing. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test reviewer_usage reviewer_usage`.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::{
    Run, TokenUsage,
    review::{AutoModeSettings, DEFAULT_REVIEW_MODEL, ReviewEnd, ReviewRecord, ReviewerChoice},
  },
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, TestProject, project, scheduler_with, wait_for};

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

/// A run of `project`'s task in auto mode with `reviewer` (the default one when `None`), once it ended.
async fn ended_run(project: &TestProject, reviewer: Option<ReviewerChoice>) -> (Run, ProjectDb) {
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let settings = AutoModeSettings {
    new_runs: true,
    reviewer,
  };
  db.save_auto_mode_settings(&settings).expect("auto mode for new runs");
  let (scheduler, mut events): (_, mpsc::UnboundedReceiver<Event>) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  (run, db)
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
  assert_eq!(review.model.as_str(), DEFAULT_REVIEW_MODEL);
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
