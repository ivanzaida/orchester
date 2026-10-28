//! `decisions`: what the Summary's decisions need beyond `task-states` (`Task page` `iwrwB`). A run ready
//! for review whose changes conflict with `main`, which moved on after the run started (`kxMAN`, merge
//! conflict); one that merges cleanly into the moved `main`; one started from a detached HEAD, which has
//! no branch to merge into; a run whose Review step reported a failure until its retries ran out
//! (`DVuXu`); a run whose Review could not check two states and waits unverified; and a Quick fix run whose
//! Review failed, sent the work back, and was not run again on the unchanged work (neither has a frame of its own
//! yet: both reuse `DVuXu`'s parts).

use orchester_core::{
  resource::StepKey,
  run::{Run, StepFailure, StepOutcome, StepOutput, UnverifiedCheck},
  store::ActivityStore,
  task::Task,
};

use crate::kontur::{
  Kontur, SeedError,
  catalogue::Published,
  files,
  git::{self, MAIN},
  run::Drive,
};

const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];
/// The lines of the sender and its settings that the conflicting run and `main` both change.
const SEND_LINE: &str = "    let response = self.client.post(&endpoint.url).body(body).send().await?;\n";
const TIMEOUT_LINE: &str = "  pub timeout_secs: u64,\n";
/// What Review reported when it failed the work for the last time (`RuLVp` in `DVuXu`), in Markdown as agents write
/// it: the Summary reads it as the Conversation does.
const REVIEW_REPORT: &str = concat!(
  "**Criterion 2 is still not met.** When `Retry-After` is an HTTP date, the sender reads it as seconds:\n\n",
  "1. `parse_retry_after` takes only digits.\n",
  "   - An HTTP date reads as `0`, so the sender waits 0 s.\n",
  "2. Same finding as the attempt before."
);
/// What Review reported when it could not check everything.
const UNVERIFIED_REPORT: &str =
  "Checked the task list and the empty project against the design. **Two states could not be reached.**";
/// What Review reported when it failed the work it was then not run again on.
const UNCHANGED_REPORT: &str = "The date header still shows the UTC day for runs that ended after local midnight.";
/// What it could not check, and why.
const UNVERIFIED_CHECKS: [(&str, &str); 2] = [
  (
    "Tasks · a filter that matches nothing",
    "No seed scenario has tasks that a filter leaves out entirely.",
  ),
  (
    "Task page · unreadable run",
    "The QA copy could not be started on a damaged run row.",
  ),
];

/// How a ready run was made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ready {
  /// Its Implement writes the KON-12 retry, which conflicts with what `main` gets after it.
  Conflicting,
  Clean,
  /// Started from a detached HEAD: it records no branch to merge into.
  Detached,
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let feature = &catalogue.feature_delivery;
  let ready = [
    (
      ("Add retry to webhook sender", "ready for review, conflicts with main"),
      Ready::Conflicting,
    ),
    (
      ("Show the delivery log", "ready for review, merges cleanly"),
      Ready::Clean,
    ),
    (
      ("Trim old deliveries", "ready for review, no branch to merge into"),
      Ready::Detached,
    ),
  ];
  for (title, how) in ready {
    ready_run(kontur, feature, title, how)?;
  }
  move_main_on(kontur)?;
  let task = queued(
    kontur,
    feature,
    ("Check retry delays in review", "waits after Review's retries"),
  )?;
  let at = kontur.ago(50);
  let mut drive = Drive::start(kontur, &task, feature, at)?;
  drive.fail_until_stuck("review")?;
  let run = drive.finish();
  report(kontur, &run, "review", (None, REVIEW_REPORT, Vec::new()))?;
  let task = queued(
    kontur,
    feature,
    ("Check the empty states on screen", "waits unverified after Review"),
  )?;
  let at = kontur.ago(40);
  let mut drive = Drive::start(kontur, &task, feature, at)?;
  drive.pass_before("review")?.fail("review", StepFailure::Unverified)?;
  let run = drive.finish();
  let checks = UNVERIFIED_CHECKS
    .iter()
    .map(|(check, reason)| UnverifiedCheck {
      check: (*check).to_owned(),
      reason: (*reason).to_owned(),
    })
    .collect();
  report(
    kontur,
    &run,
    "review",
    (Some(StepOutcome::Unverified), UNVERIFIED_REPORT, checks),
  )?;
  let quick_fix = &catalogue.quick_fix;
  let task = queued(
    kontur,
    quick_fix,
    ("Check the date header in review", "waits unchanged after Review failed"),
  )?;
  let at = kontur.ago(30);
  let mut drive = Drive::start(kontur, &task, quick_fix, at)?;
  drive
    .pass_before("review")?
    .fail("review", StepFailure::ReportedFailed)?;
  let failed = drive.execution("review")?;
  drive.pass_before("review")?.refuse_unchanged("review")?;
  let run = drive.finish();
  save_report(
    kontur,
    (&run, failed, "review"),
    (Some(StepOutcome::Failed), UNCHANGED_REPORT, Vec::new()),
  )
}

/// Records what `step` of `run` reported last, as the runner records an agent's final message: its outcome, its
/// summary and what it could not check.
fn report(
  kontur: &mut Kontur,
  run: &Run,
  step: &str,
  (outcome, text, unverified): (Option<StepOutcome>, &str, Vec<UnverifiedCheck>),
) -> Result<(), SeedError> {
  let key = StepKey::new(step).expect("scenarios name the catalogue's step keys");
  let execution = run
    .latest_execution(&key)
    .ok_or_else(|| SeedError::NoSuchStep(step.to_owned()))?
    .id();
  save_report(kontur, (run, execution, step), (outcome, text, unverified))
}

/// Records what `execution` of `step` in `run` reported.
fn save_report(
  kontur: &mut Kontur,
  (run, execution, step): (&Run, orchester_core::id::StepExecutionId, &str),
  (outcome, text, unverified): (Option<StepOutcome>, &str, Vec<UnverifiedCheck>),
) -> Result<(), SeedError> {
  let output = StepOutput {
    run: run.id(),
    execution,
    step: StepKey::new(step).expect("scenarios name the catalogue's step keys"),
    outcome,
    text: text.to_owned(),
    unverified,
    created_at: kontur.ago(45),
  };
  kontur.db.save_step_output(&output)?;
  Ok(())
}

/// A task whose run passed every step, made `how`.
fn ready_run(
  kontur: &mut Kontur,
  pipeline: &Published,
  title: (&str, &'static str),
  how: Ready,
) -> Result<(), SeedError> {
  let task = queued(kontur, pipeline, title)?;
  let at = kontur.ago(150);
  let mut drive = match how {
    Ready::Detached => Drive::start_detached(kontur, &task, pipeline, at)?,
    Ready::Conflicting | Ready::Clean => Drive::start(kontur, &task, pipeline, at)?,
  };
  if how == Ready::Conflicting {
    drive
      .pass_before("implement")?
      .pass_with("implement", &files::retry_checkpoint())?;
  }
  drive.pass_all()?;
  Ok(())
}

/// A queued task titled `title`, noted in the seed's report as left in `state`.
fn queued(kontur: &mut Kontur, pipeline: &Published, (title, state): (&str, &'static str)) -> Result<Task, SeedError> {
  let created = kontur.ago(180);
  let mut task = kontur.task((title, state, &CRITERIA), Some(pipeline), created)?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, pipeline)?;
  Ok(task)
}

/// `main` moves on after the runs started: the sender and its settings change where the conflicting run
/// changes them. Gives `main`'s new head.
pub(super) fn move_main_on(kontur: &Kontur) -> Result<orchester_core::revision::Revision, SeedError> {
  let initial = files::initial();
  let file = |path: &str| {
    initial
      .iter()
      .find(|(name, _)| *name == path)
      .map(|(_, content)| content.clone())
      .ok_or_else(|| SeedError::Invalid(format!("the repository has no {path}")))
  };
  let sender = file("src/webhooks/sender.rs")?.replace(
    SEND_LINE,
    "    let request = self.client.post(&endpoint.url).body(body);\n    let response = request.timeout(endpoint.timeout()).send().await?;\n",
  );
  let settings = file("src/webhooks/settings.rs")?.replace(
    TIMEOUT_LINE,
    "  pub timeout_secs: u64,\n  /// Deliveries that wait longer give up.\n  pub max_wait_secs: u64,\n",
  );
  let changed = [
    ("src/webhooks/sender.rs", sender),
    ("src/webhooks/settings.rs", settings),
  ];
  Ok(git::commit(
    &kontur.repo,
    &changed,
    &format!("Kontur: deliveries time out on {MAIN}"),
  )?)
}
