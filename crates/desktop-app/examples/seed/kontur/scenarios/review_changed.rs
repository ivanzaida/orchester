//! `review-changed`: a run waiting for review whose branch moved after the head reviewed (ORC-699, `Review · redesign`
//! 09 `Os7b3` `Row · KON-20` and 09i `wLyFt`). *Sign webhook payloads* passed every step on Quick fix with the retry
//! work of KON-12 at its last checkpoint, then two commits landed on its branch with no checkpoint. Its row in Review ›
//! Merges reads *Changed since your review: 2 commits after <checkpoint> · nothing merged · review again*; its review
//! page, opened from the row or at `/project/<id>/review/<KEY>`, says so under its status line with a strip that
//! switches the diff between all changes and those since the review (ORC-798). The first of the two commits rewrites a
//! line a comment of the review is on, so that comment reads Outdated.

use std::num::NonZeroU32;

use orchester_core::{
  id::{ReviewMessageId, ReviewThreadId},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  review::{CommentState, ReviewMessage, ReviewThread, Said, ThreadKind, ThreadParts},
};

use crate::kontur::{Kontur, SeedError, files, run::Drive};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
const NOTES: &str = "notes/retry.md";
const SETTINGS: &str = "src/webhooks/settings.rs";
/// The doc line of the retry budget that the checkpoint writes and the first commit after the review rewrites.
const BUDGET_DOC: &str = "  /// Attempts per delivery, the first included.";
const BUDGET_DOC_AFTER: &str = "  /// Attempts per delivery, the first one included.";

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
  let work = files::retry_checkpoint();
  drive
    .pass_before("implement")?
    .pass_with("implement", &work)?
    .pass_all()?;
  let reviewed = drive
    .run()
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone())
    .ok_or_else(|| SeedError::Invalid("the run committed nothing".to_owned()))?;
  let settings = work
    .iter()
    .find(|(path, _)| *path == SETTINGS)
    .map(|(_, content)| content.clone())
    .ok_or_else(|| SeedError::Invalid("the work does not change the settings".to_owned()))?;
  let budget_line = settings
    .lines()
    .position(|line| line == BUDGET_DOC)
    .and_then(|index| u32::try_from(index + 1).ok())
    .ok_or_else(|| SeedError::Invalid("the settings gained no budget doc".to_owned()))?;
  drive
    .commit_past_review(
      "Cap the delay and its jitter together",
      &[
        (
          NOTES,
          "The delay and its jitter stay under 30 s together.
"
          .to_owned(),
        ),
        (SETTINGS, settings.replace(BUDGET_DOC, BUDGET_DOC_AFTER)),
      ],
    )?
    .commit_past_review(
      "Test the 30 s bound",
      &[(
        NOTES,
        "The delay and its jitter stay under 30 s together.
A test holds the bound.
"
        .to_owned(),
      )],
    )?;
  let run = drive.finish();
  comment_on_budget_doc(kontur, &run, (&reviewed, budget_line))
}

/// A comment not sent yet on the budget doc, written at the head reviewed: the first commit after it rewrites that
/// line, so the comment reads Outdated.
fn comment_on_budget_doc(
  kontur: &mut Kontur,
  run: &orchester_core::run::Run,
  (reviewed, line): (&orchester_core::revision::Revision, u32),
) -> Result<(), SeedError> {
  let invalid = |error: String| SeedError::Invalid(error);
  let line = NonZeroU32::new(line).ok_or_else(|| invalid("a line 0".to_owned()))?;
  let anchor = LineAnchor::new(
    SETTINGS,
    DiffSide::New,
    LineSpan::line(line),
    vec![BUDGET_DOC.to_owned()],
  )
  .map_err(|error| invalid(error.to_string()))?;
  let at = kontur.today_ago(60);
  let said = Said::Comment(CommentState::NotSent { failure: None });
  let words = "Say whether the first attempt counts against the budget.".to_owned();
  let parts = ThreadParts {
    id: ReviewThreadId::generate(),
    run: run.id(),
    kind: ThreadKind::Comment,
    anchor,
    head: reviewed.clone(),
    created_at: at,
    resolved_at: None,
  };
  let message = ReviewMessage::restore(ReviewMessageId::generate(), said, words, at);
  let thread = ReviewThread::new(parts, vec![message]).map_err(|error| invalid(error.to_string()))?;
  kontur.db.add_review_thread(&thread)?;
  Ok(())
}
