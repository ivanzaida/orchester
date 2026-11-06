//! `review-changed`: a run waiting for review whose branch moved after the head reviewed (ORC-699, `Review · redesign`
//! 09 `Os7b3` `Row · KON-20` and 09i `wLyFt`). *Sign webhook payloads* passed every step on Quick fix with the retry
//! work of KON-12 at its last checkpoint, then two commits landed on its branch with no checkpoint. Its row in Review ›
//! Merges reads *Changed since your review: 2 commits after <checkpoint> · nothing merged · review again*; its review
//! page, opened from the row or at `/project/<id>/merges/<KEY>/1`, says so above the diff, lists the two commits, and
//! keeps *Approve and merge* unavailable until the person took them in.

use crate::kontur::{Kontur, SeedError, files, run::Drive};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
const NOTES: &str = "notes/retry.md";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let quick = &catalogue.quick_fix;
  let state = "changed since the review";
  let mut task = kontur.task(
    ("Sign webhook payloads", state, &CRITERIA),
    Some(quick),
    kontur.ago(600),
  )?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, quick)?;
  let at = kontur.today_ago(180);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  drive
    .pass_before("implement")?
    .pass_with("implement", &files::retry_checkpoint())?
    .pass_all()?;
  drive
    .commit_past_review(
      "Cap the delay and its jitter together",
      &[(NOTES, "The delay and its jitter stay under 30 s together.\n".to_owned())],
    )?
    .commit_past_review(
      "Test the 30 s bound",
      &[(
        NOTES,
        "The delay and its jitter stay under 30 s together.\nA test holds the bound.\n".to_owned(),
      )],
    )?;
  Ok(())
}
