//! `cards-auto-mode`: one task per state of a command auto mode reviews, each a Feature delivery run with auto mode on,
//! at work on Implement: a read-only command its reviewer is still judging (the quiet *Auto mode is checking this
//! command…* row, with *Answer myself*); one the reviewer left to the person with its reason, and one whose review
//! ran out of time, both asking the person with the whole card; and one the reviewer approved (*Approved by auto mode
//! at …*).
//!
//! Its runs are recorded, not executed, as the `cards` scenario's are: so that the requests still under review or
//! asking can be seen without a runner, their step is recorded waiting for the person (`cards::hold`), which the
//! runner itself never does for a request. That hold makes those runs read as waiting for the person (the toolbar's
//! *Waiting for you*, *Needs you*) in this scenario only; a run whose request is under review reads as at work.

use orchester_core::{
  run::{
    DecidedBy, Invocation, PendingToolApproval, Run, ToolApprovalDecision,
    review::{Review, ReviewFailure},
  },
  time::Timestamp,
};

use super::cards::{at_work, hold};
use crate::kontur::{Kontur, SeedError};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
/// The agent that asks, Implement's.
const AGENT: &str = "Builder";
/// The model auto mode reviewed with.
const REVIEWER: &str = "claude-haiku-4-5";

/// Where auto mode's review of a request stands.
#[derive(Clone, Copy)]
enum Reviewed {
  /// The reviewer is still judging it.
  UnderReview,
  /// The reviewer left it to the person, for this reason.
  Asked(&'static str),
  /// The review ran out of time; the person decides.
  TimedOut,
  /// The reviewer approved it.
  Approved,
}

const TASKS: [(&str, &str, Reviewed, &str); 4] = [
  (
    "Show the changelog's last lines",
    "tail -n 3 CHANGELOG.md",
    Reviewed::UnderReview,
    "command under auto mode's review",
  ),
  (
    "Bump the HTTP client",
    "cargo update -p reqwest",
    Reviewed::Asked("It changes Cargo.lock beyond the task's scope; the person should decide."),
    "command auto mode left to you",
  ),
  (
    "List the duplicate crates",
    "cargo tree -d",
    Reviewed::TimedOut,
    "command whose review timed out",
  ),
  (
    "Read the webhook test log",
    "tail -n 40 target/webhook-tests.log",
    Reviewed::Approved,
    "command approved by auto mode",
  ),
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for (index, (title, command, reviewed, state)) in TASKS.into_iter().enumerate() {
    let minutes = i64::try_from(TASKS.len() - index).expect("a short list") * 30;
    let (created, started) = (kontur.ago(minutes + 20), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &CRITERIA), Some(pipeline), created)?;
    // Runs are in Manual, whose reviewer judges what the rules leave to the person.
    let (run, invocation) = at_work(kontur, &mut task, pipeline, started)?;
    kontur.seeded(&task, state);
    // Two minutes into the agent's turn.
    let at = Timestamp::from_unix_millis(invocation.started_at.unix_millis() + 2 * 60_000);
    request(kontur, (&run, &invocation), command, (reviewed, at))?;
    hold(kontur, run, (&invocation, !matches!(reviewed, Reviewed::Approved)))?;
  }
  Ok(())
}

/// A request to run `command`, asked at `at`, as auto mode's review left it.
fn request(
  kontur: &mut Kontur,
  (run, invocation): (&Run, &Invocation),
  command: &str,
  (reviewed, at): (Reviewed, Timestamp),
) -> Result<(), SeedError> {
  let request = PendingToolApproval {
    id: format!("{}:call_1", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: AGENT.to_owned(),
    command: true,
    scope: Some("The step needs it to go on.".to_owned()),
    server: "command".to_owned(),
    action: command.to_owned(),
    target: None,
    requested_at: at,
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  let model = REVIEWER.to_owned();
  let review = match reviewed {
    Reviewed::UnderReview => {
      kontur.db.set_reviewing(run.id(), &request.id, true)?;
      return Ok(());
    }
    Reviewed::Asked(reason) => Review::Asked {
      reason: reason.to_owned(),
      model,
    },
    Reviewed::TimedOut => Review::Failed {
      failure: ReviewFailure::TimedOut,
      model: Some(model),
      detail: Some("the reviewer did not answer in time".to_owned()),
    },
    Reviewed::Approved => Review::Approved {
      reason: "It only reads a log the step's tests wrote.".to_owned(),
      model,
    },
  };
  kontur.db.record_review(run.id(), &request.id, &review)?;
  if review.approves() {
    let decided = Timestamp::from_unix_millis(at.unix_millis() + 12_000);
    kontur
      .db
      .claim_tool_approval(run.id(), &request.id, DecidedBy::Reviewer, decided)?;
    kontur
      .db
      .delivered_tool_approval(run.id(), &request.id, ToolApprovalDecision::Approved)?;
  }
  Ok(())
}
