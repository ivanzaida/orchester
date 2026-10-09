//! `decisions`: what the Summary's decisions need beyond `task-states` (`Task page` `iwrwB`). A run ready
//! for review whose changes conflict with `main`, which moved on after the run started (`kxMAN`, merge
//! conflict); one that merges cleanly into the moved `main`; one started from a detached HEAD, which has
//! no branch to merge into; and a run whose Review step reported a failure until its retries ran out
//! (`DVuXu`).

use orchester_core::{
  resource::StepKey,
  run::{Run, StepOutput},
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
/// What Review reported when it failed the work for the last time (`RuLVp` in `DVuXu`).
const REVIEW_REPORT: &str = "Criterion 2 is still not met: when Retry-After is an HTTP date, the sender reads it as seconds                              and waits 0 s. Same finding as the attempt before.";

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
  report(kontur, &run, "review", REVIEW_REPORT)
}

/// Records what `step` of `run` reported last, as the runner records an agent's final message.
fn report(kontur: &mut Kontur, run: &Run, step: &str, text: &str) -> Result<(), SeedError> {
  let key = StepKey::new(step).expect("scenarios name the catalogue's step keys");
  let execution = run
    .latest_execution(&key)
    .ok_or_else(|| SeedError::NoSuchStep(step.to_owned()))?
    .id();
  let output = StepOutput {
    run: run.id(),
    execution,
    step: key,
    outcome: None,
    text: text.to_owned(),
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
/// changes them.
fn move_main_on(kontur: &Kontur) -> Result<(), SeedError> {
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
  git::commit(
    &kontur.repo,
    &changed,
    &format!("Kontur: deliveries time out on {MAIN}"),
  )?;
  Ok(())
}
