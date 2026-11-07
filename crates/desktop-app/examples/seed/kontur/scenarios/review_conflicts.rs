//! `review-conflicts`: an approved run whose merge into `main` found conflicts and merged nothing (ORC-807,
//! `Review - v2` `j1Drb` 11 `CT6Gc`). *Rotate API keys from settings* passed every step on Quick fix with the retry
//! work of KON-12 as its checkpoint; then `main` changed the lines of `sender.rs` and `settings.rs` that checkpoint
//! changes, and the merge the person asked for conflicted in both. Its row in Review › Waiting for you reads *Conflicts
//! with main*; its review, at `/project/<id>/review/<KEY>`, marks the two files, shows what `main` has under the
//! conflicting lines and holds the conflict bar in the tray's place. Start the QA app without a runner so it stays as
//! seeded, or with the fake agents to press *Ask the agent to resolve*.

use orchester_core::integration::attempt::AttemptOutcome;

use super::merges::{PERSON, record};
use crate::kontur::{Kontur, SeedError, files, git, run::Drive};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let quick = &catalogue.quick_fix;
  let state = "conflicts with the base";
  let mut task = kontur.task(
    ("Rotate API keys from settings", state, &CRITERIA),
    Some(quick),
    kontur.ago(600),
  )?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, quick)?;
  let at = kontur.today_ago(120);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  drive
    .pass_before("implement")?
    .pass_with("implement", &files::retry_checkpoint())?
    .pass_all()?;
  let run = drive.finish();
  git::commit(
    &kontur.repo,
    &files::main_moved_on(),
    "Log every delivery, pause endpoints",
  )?;
  let files = vec![
    "src/webhooks/sender.rs".to_owned(),
    "src/webhooks/settings.rs".to_owned(),
  ];
  record(
    kontur,
    &run,
    (kontur.today_ago(45), PERSON),
    (AttemptOutcome::Conflicts { files }, None),
  )
}
