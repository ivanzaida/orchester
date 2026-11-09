//! `review-approved`: an approved task's review, read-only (ORC-800, `Review - v2` 07 `eidig`), each opened from its
//! row in *Approved today* or at `/project/<id>/review/<KEY>`.
//!
//! - *Retry budget per endpoint*: the retry work of KON-12, merged from its task page by the person today, its branch
//!   deleted as Orchester's cleanup does. Its threads were kept: a comment Implement addressed, a question it answered,
//!   and a comment that was never sent.
//! - *Honor Retry-After on 429 and 503*: merged today by automation's policy, so no person approved it.

use orchester_core::review::{CommentState, QuestionState, Said, ThreadKind};

use super::{
  merges::{AUTOMATION, PERSON, branch_deleted, merged_today, record_landed},
  review_page::{SENDER, Seeded, added, checkpoint, content, queued, store, words},
};
use crate::kontur::{Kontur, SeedError, catalogue::Published, files, run::Drive};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let quick = &catalogue.quick_fix;
  merged_with_threads(kontur, quick)?;
  let run = merged_today(
    kontur,
    quick,
    ("Honor Retry-After on 429 and 503", 200),
    (AUTOMATION, None),
  )?;
  branch_deleted(kontur, &run)
}

fn merged_with_threads(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let task = queued(
    kontur,
    quick,
    ("Retry budget per endpoint", "merged today with its review kept"),
  )?;
  let at = kontur.today_ago(220);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  let work = files::retry_checkpoint();
  drive
    .pass_before("implement")?
    .pass_with("implement", &work)?
    .pass_all()?;
  let head = checkpoint(drive.run())?;
  let sender = added(&content(&work, SENDER)?);
  let line = |at: usize| {
    sender
      .get(at)
      .cloned()
      .ok_or_else(|| SeedError::Invalid("the sender gained fewer lines".to_owned()))
  };
  let threads = [
    (
      ThreadKind::Comment,
      line(7)?,
      vec![
        (
          Said::Comment(CommentState::Addressed {
            commit: Some(head.clone()),
          }),
          "Cap the sum of the delay and the jitter, not only the delay.",
        ),
        (
          words(false, None),
          "Capped the sum: the delay and its jitter together stay under MAX_RETRY_DELAY. Added a test for the bound.",
        ),
      ],
    ),
    (
      ThreadKind::Question,
      line(0)?,
      vec![
        (
          Said::Question(QuestionState::Answered),
          "Is the first attempt counted in the budget?",
        ),
        (
          words(false, None),
          "The budget counts every attempt, the first included, so 5 means four retries.",
        ),
      ],
    ),
    (
      ThreadKind::Comment,
      line(2)?,
      vec![(
        Said::Comment(CommentState::NotSent { failure: None }),
        "Log the attempt before the request goes, not after it returns.",
      )],
    ),
  ];
  drive.merge()?;
  let run = drive.finish();
  record_landed(kontur, &run, PERSON)?;
  for (minutes, (kind, (number, text), messages)) in (0_i64..).zip(threads) {
    let seeded = Seeded {
      kind,
      lines: (SENDER.to_owned(), number, number, vec![text]),
      head: head.clone(),
      messages: messages
        .into_iter()
        .map(|(said, words)| (said, words.to_owned()))
        .collect(),
      resolved: false,
    };
    store(kontur, &run, seeded, kontur.today_ago(200 - 2 * minutes))?;
  }
  branch_deleted(kontur, &run)
}
