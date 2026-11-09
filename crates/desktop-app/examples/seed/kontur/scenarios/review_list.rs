//! `review-list`: Review's list (ORC-794, `Review - v2` `j1Drb` 01 `i6S42`): *Waiting for you* with a run whose comment
//! is not sent, a run whose branch moved after the head reviewed and an approved run whose merge found conflicts;
//! *With the agent* with a run Implement works on a comment of; *Approved today* with two runs merged today. Start the
//! QA app without a runner so the run Implement works on stays as it is seeded.

use orchester_core::integration::attempt::AttemptOutcome;

use super::{
  merges::{PERSON, branch_deleted, merged_today, ready, record, unsent_comments, waiting_in_queue},
  review_page,
};
use crate::kontur::{Kontur, SeedError, run::Drive};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let (feature, quick) = (&catalogue.feature_delivery, &catalogue.quick_fix);
  let queued = waiting_in_queue(kontur, feature, "Sign webhook payloads")?;
  unsent_comments(kontur, &queued, 1)?;
  let mut task = kontur.task(
    ("Endpoint health page", "changed since the review", &CRITERIA),
    Some(quick),
    kontur.ago(600),
  )?;
  kontur.seeded(&task, "changed since the review");
  kontur.queue(&mut task, quick)?;
  let at = kontur.today_ago(160);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  drive.pass_all()?;
  drive.commit_past_review(
    "Check the endpoint's TLS too",
    &[("notes/health.md", "Health page: one more check.\n".to_owned())],
  )?;
  let (_, conflicting) = ready(
    kontur,
    quick,
    ("Rotate API keys from settings", "conflicts with the base"),
    120,
  )?;
  let at = kontur.today_ago(45);
  let files = vec![
    "src/auth/keys.rs".to_owned(),
    "src/auth/settings.rs".to_owned(),
    "src/auth/mod.rs".to_owned(),
  ];
  record(
    kontur,
    &conflicting,
    (at, PERSON),
    (AttemptOutcome::Conflicts { files }, None),
  )?;
  review_page::at_work(kontur, quick)?;
  for (title, ago) in [
    ("Retry budget per endpoint", 260),
    ("Honor Retry-After on 429 and 503", 330),
  ] {
    let run = merged_today(kontur, quick, (title, ago), (PERSON, None))?;
    branch_deleted(kontur, &run)?;
  }
  Ok(())
}
