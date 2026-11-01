//! `cards-approval-modes`: how each tool request was answered by the run's approval mode, and why one came to the
//! person (`docs/design/approval-modes.md`, 04 `H6kPd`, 04b `p6tXIO`, 04c `vZcoN`). Each task is a Feature delivery
//! run at work on Implement:
//!
//! - *Add retry to webhook sender*: six commands Autopilot approved (three rows and *· 3 more*), then a file outside
//!   the worktree that came to the person, with its path;
//! - *Cap Retry-After at 60 s*: a command the profile allowed, one the reviewer approved with its reason, then one the
//!   reviewer was not sure about;
//! - one task per other reason a request comes to the person: the reviewer denied it, it is not on the allowed list, a
//!   push to a remote, another branch, the reviewer could not run (ORC-611: its review failed, and the reviewer's
//!   account is at its weekly plan limit, which resets two days after the seed's now — Project settings › Runs ›
//!   Reviewer says so too);
//! - the reviewer did not answer in time, and its answer could not be read (ORC-586);
//! - an MCP call of an Autopilot step the reviewer was not sure about, and one of a Manual step not on its allowed list
//!   (ORC-586: only Manual's headline says "not on its allowed list").
//!
//! Its runs are recorded, not executed, as `cards-auto-mode`'s are: a step with a request that came to the person is
//! recorded waiting for them (`cards::hold`).

use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  run::{
    AnswerKind, CameToPerson, DecidedBy, Invocation, PendingToolApproval, PersonReason, RequestAnswer, Run,
    ToolApprovalDecision,
    review::{Review, ReviewFailure, resolve_reviewer},
  },
  store::ConnectionStore,
  time::Timestamp,
};

use super::cards::{at_work, hold};
use crate::kontur::{Kontur, SeedError, catalogue::recorded_connection};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
/// The agent that asks, Implement's.
const AGENT: &str = "Builder";

/// How the run's mode answered a request.
#[derive(Clone, Copy)]
enum Answered {
  /// Approved without the person: by Autopilot, the profile or the reviewer (with its reason).
  Approved(AnswerKind, Option<&'static str>),
  /// It came to the person for this reason, with what the card names.
  Came(PersonReason, Option<&'static str>),
  /// The same, by Autopilot's rules.
  CameInAutopilot(PersonReason, Option<&'static str>),
  /// It came to the person as the reviewer could not judge it: its review failed so.
  NotJudged(ReviewFailure),
}

/// A command, or an MCP call written `server::tool`.
type Asked = (&'static str, Answered);

const AUTOPILOT: [Asked; 7] = [
  (
    "cargo build -p webhooks",
    Answered::Approved(AnswerKind::Autopilot, None),
  ),
  (
    "cargo test -p webhooks retry",
    Answered::Approved(AnswerKind::Autopilot, None),
  ),
  ("cargo fmt -p webhooks", Answered::Approved(AnswerKind::Autopilot, None)),
  (
    "cargo clippy -p webhooks",
    Answered::Approved(AnswerKind::Autopilot, None),
  ),
  ("git diff --stat", Answered::Approved(AnswerKind::Autopilot, None)),
  (
    "cargo test -p webhooks",
    Answered::Approved(AnswerKind::Autopilot, None),
  ),
  (
    "Add-Content 'C:\\Users\\dana\\.cargo\\config.toml' 'net.retry = 5'",
    Answered::Came(PersonReason::Outside, Some("C:\\Users\\dana\\.cargo\\config.toml")),
  ),
];

const MANUAL: [Asked; 3] = [
  ("cargo test -p webhooks", Answered::Approved(AnswerKind::Profile, None)),
  (
    "rm -rf target/fixtures-tmp",
    Answered::Approved(
      AnswerKind::Reviewer,
      Some("Deletes generated fixtures inside the worktree."),
    ),
  ),
  (
    "curl -sI https://staging.example.com/webhooks",
    Answered::Came(
      PersonReason::ReviewerUnsure,
      Some("Sends a request to an outside host; can't tell whether it only tests the retry path."),
    ),
  ),
];

const DENIED: [Asked; 1] = [(
  "cargo add reqwest --features json",
  Answered::Came(
    PersonReason::ReviewerDenied,
    Some("Adds a new dependency the task doesn't ask for."),
  ),
)];
const NOT_ALLOWED: [Asked; 1] = [(
  "cat ~/.ssh/id_ed25519.pub",
  Answered::Came(PersonReason::NotAllowed, None),
)];
const PUSH: [Asked; 1] = [("git push origin HEAD", Answered::Came(PersonReason::Push, None))];
const REFS: [Asked; 1] = [(
  "git update-ref refs/heads/main 4e1c9d2",
  Answered::Came(PersonReason::Refs, Some("refs/heads/main")),
)];
const NOT_JUDGED: [Asked; 1] = [(
  "curl -s https://staging.example.com/uploads/health",
  Answered::NotJudged(ReviewFailure::CouldNotRun),
)];
const TIMED_OUT: [Asked; 1] = [(
  "curl -s https://staging.example.com/downloads/health",
  Answered::NotJudged(ReviewFailure::TimedOut),
)];
const UNREADABLE: [Asked; 1] = [(
  "cargo update -p serde_json",
  Answered::NotJudged(ReviewFailure::Unreadable),
)];
const AUTOPILOT_TOOL: [Asked; 1] = [(
  "pencil::batch_design",
  Answered::CameInAutopilot(
    PersonReason::ReviewerUnsure,
    Some("Can't tell which files the design tool writes."),
  ),
)];
const MANUAL_TOOL: [Asked; 1] = [("pencil::batch_design", Answered::Came(PersonReason::NotAllowed, None))];

const TASKS: [(&str, &[Asked], &str); 11] = [
  (
    "Add retry to webhook sender",
    &AUTOPILOT,
    "Autopilot rows and a file outside the worktree",
  ),
  (
    "Cap Retry-After at 60 s",
    &MANUAL,
    "profile and reviewer rows and the reviewer unsure",
  ),
  ("Add a JSON client", &DENIED, "a command the reviewer denied"),
  ("Read the deploy key", &NOT_ALLOWED, "a command not on the allowed list"),
  ("Publish the retry branch", &PUSH, "a push to a remote"),
  ("Move main to the fix", &REFS, "a change to another branch"),
  (
    "Retry uploads on 503",
    &NOT_JUDGED,
    "a command the reviewer could not judge: its account is at its weekly plan limit",
  ),
  (
    "Retry downloads on 503",
    &TIMED_OUT,
    "a command the reviewer could not judge: it did not answer in time",
  ),
  (
    "Bump the JSON parser",
    &UNREADABLE,
    "a command the reviewer could not judge: its answer could not be read",
  ),
  (
    "Redraw the retry banner",
    &AUTOPILOT_TOOL,
    "an Autopilot step's MCP call the reviewer was not sure about",
  ),
  (
    "Sketch the retry banner",
    &MANUAL_TOOL,
    "a Manual step's MCP call not on the allowed list",
  ),
];

/// When the reviewer's account's weekly limit resets: two days after the seed's now.
const RESETS_IN_MINUTES: i64 = 2 * 24 * 60;

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  // The project's reviewer: a Claude Code connection on its CLI's Default account, recorded and never run.
  let program = kontur.repo.join(".orchester").join("recorded").join("claude");
  let published = kontur.ago(7 * 24 * 60);
  recorded_connection(
    &mut kontur.db,
    ("Claude · Claude 2", AgentProvider::ClaudeCodeCli, &program),
    published,
  )?;
  for (index, (title, asked, state)) in TASKS.into_iter().enumerate() {
    let minutes = i64::try_from(TASKS.len() - index).expect("a short list") * 30;
    let (created, started) = (kontur.ago(minutes + 20), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), created)?;
    let (run, invocation) = at_work(kontur, &mut task, pipeline, started)?;
    kontur.seeded(&task, state);
    let mut waits = false;
    for (call, (command, answered)) in asked.iter().enumerate() {
      // A minute into the agent's turn, then 15 s apart.
      let offset = 60_000 + i64::try_from(call).expect("a short list") * 15_000;
      let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + offset);
      let request = request((&run, &invocation), (call, command), at);
      kontur.db.request_tool_approval(&request)?;
      waits |= answer(kontur, &request, *answered)?;
    }
    hold(kontur, run, (&invocation, waits))?;
  }
  Ok(())
}

/// The `call`-th request of `invocation` to run `asked`, a command or a `server::tool` call, asked at `at`.
fn request((run, invocation): (&Run, &Invocation), (call, asked): (usize, &str), at: Timestamp) -> PendingToolApproval {
  let (command, server, action) = match asked.split_once("::") {
    Some((server, tool)) => (false, server, tool),
    None => (true, "command", asked),
  };
  PendingToolApproval {
    id: format!("{}:call_{}", invocation.id, call + 1),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command,
    scope: None,
    server: server.to_owned(),
    action: action.to_owned(),
    target: None,
    requested_at: at,
    native: None,
  }
}

/// Records how the run's mode answered `request`; whether it waits for the person.
fn answer(kontur: &mut Kontur, request: &PendingToolApproval, answered: Answered) -> Result<bool, SeedError> {
  let run = request.run;
  let came = match answered {
    Answered::Approved(kind, reason) => {
      let decided_by = match kind {
        AnswerKind::Profile => DecidedBy::Permissions,
        AnswerKind::Reviewer => DecidedBy::Reviewer,
        AnswerKind::Autopilot | AnswerKind::Person => DecidedBy::AutoApproved,
      };
      let decided = Timestamp::from_unix_millis(request.requested_at.unix_millis() + 2_000);
      kontur.db.claim_tool_approval(run, &request.id, decided_by, decided)?;
      let answer = reason.map_or_else(|| RequestAnswer::by(kind), RequestAnswer::reviewer);
      kontur.db.record_request_answer(run, &request.id, &answer)?;
      kontur
        .db
        .delivered_tool_approval(run, &request.id, ToolApprovalDecision::Approved)?;
      return Ok(false);
    }
    Answered::Came(reason, detail) => CameToPerson {
      detail: detail.map(str::to_owned),
      ..CameToPerson::because(reason)
    },
    Answered::CameInAutopilot(reason, detail) => CameToPerson {
      detail: detail.map(str::to_owned),
      ..CameToPerson::because(reason)
    }
    .in_mode(true),
    Answered::NotJudged(failure) => {
      not_judged(kontur, request, failure)?;
      CameToPerson::reviewer_failed(failure)
    }
  };
  kontur.db.record_came_to_person(run, &request.id, Some(&came))?;
  Ok(true)
}

/// The reviewer's review of `request` that failed so; for a CLI that failed, as one at its account's plan limit does,
/// the account's weekly limit reached, as the CLI reported it then.
fn not_judged(kontur: &mut Kontur, request: &PendingToolApproval, failure: ReviewFailure) -> Result<(), SeedError> {
  let failed = Review::Failed {
    failure,
    model: Some("claude-haiku-4-5".to_owned()),
  };
  kontur.db.record_review(request.run, &request.id, &failed)?;
  if failure != ReviewFailure::CouldNotRun {
    return Ok(());
  }
  let (settings, connections) = (kontur.db.auto_mode_settings()?, kontur.db.connections()?);
  let reviewer = resolve_reviewer(&settings, &connections).expect("the catalogue's connection reviews");
  let provider = reviewer.kind.provider();
  let week = LimitWindow::new(
    7 * 24 * 60,
    Some(100.0),
    Some(kontur.ago(-RESETS_IN_MINUTES)),
    LimitState::Reached,
    false,
  )
  .expect("a whole week, used up");
  let reading = LimitReading {
    provider,
    account: reviewer.connection.account().id(provider),
    at: request.requested_at,
    limits: PlanLimits {
      windows: vec![week],
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  kontur.db.save_limit_snapshot(request.run, &reading)?;
  Ok(())
}
