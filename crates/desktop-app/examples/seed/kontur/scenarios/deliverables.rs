//! `deliverables`: runs whose result is not a commit (ORC-93). Feature delivery runs that passed every step and
//! committed nothing: one whose task delivers a report, ready to be completed; one whose task delivers work outside the
//! repository, with a Pencil tool call a person approved; one whose pipeline expects changes, flagged; and a report run
//! a person completed, its report kept as the run's *Report*. None has a design frame of its own yet: they reuse the
//! Ready card (`hyzq2`), the warning notice (`iw9rS`) and the Summary's sections.

use orchester_core::{
  resource::{Deliverable, StepKey},
  run::{
    CompletedBy, DecidedBy, PendingToolApproval, REPORT_ARTIFACT, Run, RunArtifact, RunCompletion, StepOutcome,
    StepOutput, ToolApprovalDecision,
  },
  store::ActivityStore,
  task::organization::ChangeActor,
};

use crate::kontur::{Kontur, SeedError, catalogue::Published, run::Drive};

const CRITERIA: [&str; 2] = ["The report names the cause.", "Nothing in the repository changes."];
/// What each run's Review reported last: its report.
const SLOW_EXPORT: &str = "## Cause\nThe export reads every contact's deliveries one query at a time.\n\n## Fix\nRead \
                           them in one query per page; no repository change was asked for.";
const SIGN_IN: &str = "Drew the sign-in page in Pencil: the form, its error state and the locked account notice, in \
                       `sign-in.pen` › *Sign in · dark*.";
const TIMEOUT: &str = "The timeout is already configurable; nothing needed to change.";
const RETRY_POLICY: &str = "## Retry policy\nRetries back off from 1 s to 5 min and give up after six tries. It matches \
                            the brief.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let feature = &catalogue.feature_delivery;
  nothing_committed(
    kontur,
    feature,
    (
      ("Find why exports are slow", "report run ready to complete"),
      Some(Deliverable::Report),
    ),
    (SLOW_EXPORT, 120),
  )?;
  let outside = nothing_committed(
    kontur,
    feature,
    (
      ("Design the sign-in page", "outside run ready to complete"),
      Some(Deliverable::Outside),
    ),
    (SIGN_IN, 100),
  )?;
  tool_call(kontur, &outside, ("pencil", "batch_design", "sign-in.pen"))?;
  nothing_committed(
    kontur,
    feature,
    (("Fix the export timeout", "committed nothing, changes expected"), None),
    (TIMEOUT, 80),
  )?;
  let completed = nothing_committed(
    kontur,
    feature,
    (
      ("Review the retry policy", "report run completed"),
      Some(Deliverable::Report),
    ),
    (RETRY_POLICY, 60),
  )?;
  let at = kontur.ago(20);
  let report = RunArtifact {
    run: completed.id(),
    name: REPORT_ARTIFACT.to_owned(),
    step: Some(review()),
    text: RETRY_POLICY.to_owned(),
    created_at: at,
  };
  let completion = RunCompletion {
    run: completed.id(),
    by: CompletedBy::Person,
    completed_at: at,
  };
  kontur.db.save_run_completion(&completion, Some(&report))?;
  Ok(())
}

fn review() -> StepKey {
  StepKey::new("review").expect("Feature delivery has a Review step")
}

/// A task titled `title`, noted as left in `state`, that delivers `delivers` (`None`: as its pipeline, changes), and
/// its run, which passed every step `ago` minutes ago committing nothing, its Review reporting `report`.
fn nothing_committed(
  kontur: &mut Kontur,
  pipeline: &Published,
  ((title, state), delivers): ((&str, &'static str), Option<Deliverable>),
  (report, ago): (&str, i64),
) -> Result<Run, SeedError> {
  let created = kontur.ago(ago + 30);
  let mut task = kontur.task((title, state, &CRITERIA), Some(pipeline), created)?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, pipeline)?;
  kontur
    .db
    .update_task_organization(task.id(), ChangeActor::Person, created, |value| {
      value.with_delivers(delivers)
    })?;
  let at = kontur.ago(ago);
  let mut drive = Drive::start(kontur, &task, pipeline, at)?;
  drive.pass_all_without_commits()?;
  let run = drive.finish();
  let execution = run
    .latest_execution(&review())
    .ok_or_else(|| SeedError::NoSuchStep("review".to_owned()))?
    .id();
  let output = StepOutput {
    run: run.id(),
    execution,
    step: review(),
    outcome: Some(StepOutcome::Done),
    text: report.to_owned(),
    unverified: Vec::new(),
    created_at: kontur.ago(ago - 5),
  };
  kontur.db.save_step_output(&output)?;
  Ok(run)
}

/// The run's Implement agent called `tool` of the MCP server `server` on `target`, which a person approved.
fn tool_call(kontur: &mut Kontur, run: &Run, (server, tool, target): (&str, &str, &str)) -> Result<(), SeedError> {
  let implement = StepKey::new("implement").expect("Feature delivery has an Implement step");
  let execution = run
    .latest_execution(&implement)
    .ok_or_else(|| SeedError::NoSuchStep("implement".to_owned()))?
    .id();
  let invocation = kontur
    .db
    .invocations(run.id())?
    .into_iter()
    .find(|invocation| invocation.execution == execution)
    .ok_or(SeedError::NotAtWork)?
    .id;
  let request = PendingToolApproval {
    id: format!("{invocation}:mcp_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: false,
    scope: Some(format!("{{\"file\":\"{target}\"}}")),
    server: server.to_owned(),
    action: tool.to_owned(),
    target: Some(target.to_owned()),
    requested_at: kontur.ago(95),
    native: None,
  };
  kontur.db.request_tool_approval(&request)?;
  kontur
    .db
    .claim_tool_approval(run.id(), &request.id, DecidedBy::Person, kontur.ago(94))?;
  kontur
    .db
    .delivered_tool_approval(run.id(), &request.id, ToolApprovalDecision::Approved)?;
  Ok(())
}
