//! `review-page`: a run's review page (ORC-697, `Review · redesign` 09i `wLyFt` and the states of `RBR9O`), each run
//! opened from its row in Review or at `/project/<id>/review/<KEY>`. Answers under way are kept only
//! while no runner drives the project: open it in a QA app started without a runner, or the runner ends them. State 8
//! *Sending…* and the tray's sending state are a send under way: call the debug tool `hold_hand_offs` `held: true`,
//! then *Send to Implement (1)* on *Sign webhook payloads* (or *Try again* on *Per-endpoint secrets*) keeps its
//! comments sending until `held: false` hands them to the runner.
//!
//! - *Sign webhook payloads*, waiting for review on its task page: the retry work of KON-12 in two checkpoints, so
//!   `src/webhooks/sender.rs`, `settings.rs`, a migration and a test change. Threads on `sender.rs`: a comment not sent
//!   (the tray idle with it), a question asked with nothing from Implement yet (3), one Implement is answering (4), a
//!   stopped answer (5), an answered question (6), a question not asked because the runner was not running (11), an
//!   addressed comment (10) and a resolved comment; on `settings.rs`, a comment a later checkpoint outdated. The
//!   migration is marked *Viewed*, so it shows folded.
//! - *Per-endpoint secrets*, waiting for review: two comments whose send the runner did not take (the tray failed).
//! - *Delivery log per endpoint*: a run returned to Implement with a comment sent, Implement at work on it (9); its
//!   Implement step wrote the run's last checkpoint, so the comment's Conversation link names it (*step 03 Implement*).

use std::num::NonZeroU32;

use orchester_core::{
  id::{ReviewMessageId, ReviewThreadId},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  review::{
    AgentWords, CommentState, QuestionState, ReviewFailure, ReviewMessage, ReviewThread, Said, ThreadKind, ThreadParts,
  },
  revision::Revision,
  run::Run,
  task::Task,
  time::Timestamp,
};

use crate::kontur::{Kontur, SeedError, catalogue::Published, files, run::Drive};

const CRITERIA: [&str; 1] = ["The change is covered by tests."];
const SENDER: &str = "src/webhooks/sender.rs";
const SETTINGS: &str = "src/webhooks/settings.rs";
const MIGRATION: &str = "migrations/0042_delivery_attempts.sql";
/// The doc line of the retry budget the first checkpoint writes and the second rewrites.
const BUDGET_DOC: &str = "  /// Attempts per delivery, the first included.";
const BUDGET_DOC_AFTER: &str = "  /// Attempts per delivery, the first one included.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.delivering_catalogue()?;
  let quick = &catalogue.quick_fix;
  every_thread(kontur, quick)?;
  failed_send(kontur, quick)?;
  at_work(kontur, quick)
}

/// What a thread is seeded with: its kind, the lines and their code, the head they were read at, and its messages.
struct Seeded {
  kind: ThreadKind,
  lines: (String, u32, u32, Vec<String>),
  head: Revision,
  messages: Vec<(Said, String)>,
  resolved: bool,
}

fn every_thread(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let task = queued(
    kontur,
    quick,
    ("Sign webhook payloads", "waits for review with every thread"),
  )?;
  let at = kontur.today_ago(180);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  let work = files::retry_checkpoint();
  drive
    .pass_before("implement")?
    .begin("implement")?
    .checkpoint("implement", &work)?;
  let first = checkpoint(drive.run())?;
  let settings = content(&work, SETTINGS)?.replace(BUDGET_DOC, BUDGET_DOC_AFTER);
  drive.checkpoint("implement", &[(SETTINGS, settings)])?;
  drive.pass_with("implement", &[])?.pass_all()?;
  let run = drive.finish();
  let head = checkpoint(&run)?;
  let sender = added(&content(&work, SENDER)?);
  let line = |at: usize| -> Result<(String, u32, u32, Vec<String>), SeedError> {
    let (number, text) = sender
      .get(at)
      .cloned()
      .ok_or_else(|| SeedError::Invalid("the sender gained fewer lines".to_owned()))?;
    Ok((SENDER.to_owned(), number, number, vec![text]))
  };
  let threads = threads_on_sender(&head, &line)?;
  let settings_line = added(&content(&work, SETTINGS)?)
    .into_iter()
    .find(|(_, text)| text == BUDGET_DOC)
    .ok_or_else(|| SeedError::Invalid("the settings gained no budget doc".to_owned()))?;
  let outdated = Seeded {
    kind: ThreadKind::Comment,
    lines: (
      SETTINGS.to_owned(),
      settings_line.0,
      settings_line.0,
      vec![settings_line.1],
    ),
    head: first,
    messages: vec![(
      not_sent(),
      "Say whether the first attempt counts against the budget.".to_owned(),
    )],
    resolved: false,
  };
  for (minutes, seeded) in (0_i64..).zip(threads.into_iter().chain([outdated])) {
    let at = kontur.today_ago(60 - minutes);
    store(kontur, &run, seeded, at)?;
  }
  let viewed = kontur.today_ago(20);
  kontur
    .db
    .mark_review_viewed((run.id(), &head, MIGRATION), Some(viewed))?;
  Ok(())
}

/// The lines of a file the run changed and their code, by the position of a line it added.
type Lines<'a> = &'a dyn Fn(usize) -> Result<(String, u32, u32, Vec<String>), SeedError>;

/// A thread to seed on the sender: its kind, the position of the added line it is on, its messages, and whether it
/// is resolved.
type Spec = (ThreadKind, usize, Vec<(Said, &'static str)>, bool);

/// Every state a thread on the sender's lines is seeded in.
fn threads_on_sender(head: &Revision, line: Lines<'_>) -> Result<Vec<Seeded>, SeedError> {
  questions()
    .into_iter()
    .chain(comments(head))
    .map(|(kind, at, messages, resolved)| {
      Ok(Seeded {
        kind,
        lines: line(at)?,
        head: head.clone(),
        messages: messages
          .into_iter()
          .map(|(said, words)| (said, words.to_owned()))
          .collect(),
        resolved,
      })
    })
    .collect()
}

/// Implement's words, `writing` or written, doing `activity`.
fn words(writing: bool, activity: Option<&str>) -> Said {
  Said::Agent(AgentWords {
    writing,
    activity: activity.map(str::to_owned),
  })
}

/// Questions: asked with nothing from Implement yet, being answered, stopped, answered, and not asked.
fn questions() -> Vec<Spec> {
  let asked = |state| Said::Question(state);
  let question = ThreadKind::Question;
  vec![
    (
      question,
      4,
      vec![(
        asked(QuestionState::Answering),
        "Does this loop ever end when the endpoint keeps answering 500?",
      )],
      false,
    ),
    (
      question,
      5,
      vec![
        (
          asked(QuestionState::Answering),
          "Why is the delay capped at MAX_RETRY_DELAY here?",
        ),
        (
          words(true, Some("reading src/webhooks/sender.rs…")),
          "Retry-After is read first, in retry_after(), and wins over the backoff up to",
        ),
      ],
      false,
    ),
    (
      question,
      6,
      vec![
        (
          asked(QuestionState::Stopped),
          "What happens when Retry-After is a date?",
        ),
        (
          words(false, None),
          "retry_after() reads both forms: seconds, and an HTTP date, which",
        ),
      ],
      false,
    ),
    (
      question,
      0,
      vec![
        (
          asked(QuestionState::Answered),
          "Is the first attempt counted in the budget?",
        ),
        (
          words(false, None),
          "The budget counts every attempt, the first included, so 5 means four retries.",
        ),
      ],
      false,
    ),
    (
      question,
      9,
      vec![(
        asked(QuestionState::NotAsked {
          failure: ReviewFailure::RunnerUnavailable,
        }),
        "Why does a network error not count the elapsed time?",
      )],
      false,
    ),
  ]
}

/// Comments: one not sent, one Implement addressed at `head`, and one resolved.
fn comments(head: &Revision) -> Vec<Spec> {
  let comment = ThreadKind::Comment;
  vec![
    (
      comment,
      2,
      vec![(
        not_sent(),
        "Log the attempt before the request goes, not after it returns.",
      )],
      false,
    ),
    (
      comment,
      7,
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
      false,
    ),
    (
      comment,
      1,
      vec![(not_sent(), "Name it attempts, as the settings do.")],
      true,
    ),
  ]
}

/// *Per-endpoint secrets*: two comments the runner did not take.
fn failed_send(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let task = queued(kontur, quick, ("Per-endpoint secrets", "its comments were not sent"))?;
  let at = kontur.today_ago(150);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  let work = files::retry_checkpoint();
  drive
    .pass_before("implement")?
    .pass_with("implement", &work)?
    .pass_all()?;
  let run = drive.finish();
  let head = checkpoint(&run)?;
  let sender = added(&content(&work, SENDER)?);
  let failed = Said::Comment(CommentState::NotSent {
    failure: Some(ReviewFailure::RunnerUnavailable),
  });
  for (at, words) in [
    (1, "Read the budget once, before the loop."),
    (3, "Time the attempt from before the request is built."),
  ] {
    let (number, text) = sender
      .get(at)
      .cloned()
      .ok_or_else(|| SeedError::Invalid("the sender gained fewer lines".to_owned()))?;
    let seeded = Seeded {
      kind: ThreadKind::Comment,
      lines: (SENDER.to_owned(), number, number, vec![text]),
      head: head.clone(),
      messages: vec![(failed.clone(), words.to_owned())],
      resolved: false,
    };
    let stamp = kontur.today_ago(40);
    store(kontur, &run, seeded, stamp)?;
  }
  Ok(())
}

/// *Delivery log per endpoint*: its comment was sent, and Implement works on it.
pub(super) fn at_work(kontur: &mut Kontur, quick: &Published) -> Result<(), SeedError> {
  let task = queued(
    kontur,
    quick,
    ("Delivery log per endpoint", "Implement works on a comment sent"),
  )?;
  let at = kontur.today_ago(120);
  let mut drive = Drive::start(kontur, &task, quick, at)?;
  let work = files::retry_checkpoint();
  drive
    .pass_before("implement")?
    .begin("implement")?
    .checkpoint("implement", &work)?;
  let run = drive.finish();
  let head = checkpoint(&run)?;
  let (number, text) = added(&content(&work, SENDER)?)
    .into_iter()
    .nth(2)
    .ok_or_else(|| SeedError::Invalid("the sender gained fewer lines".to_owned()))?;
  let sent = kontur.today_ago(10);
  let seeded = Seeded {
    kind: ThreadKind::Comment,
    lines: (SENDER.to_owned(), number, number, vec![text]),
    head,
    messages: vec![(
      Said::Comment(CommentState::Sent { at: sent }),
      "Log the attempt before the request goes, not after it returns.".to_owned(),
    )],
    resolved: false,
  };
  store(kontur, &run, seeded, kontur.today_ago(12))
}

/// A queued task titled `title`, on `pipeline`, noted as left in `state`.
fn queued(kontur: &mut Kontur, pipeline: &Published, (title, state): (&str, &'static str)) -> Result<Task, SeedError> {
  let mut task = kontur.task((title, state, &CRITERIA), Some(pipeline), kontur.ago(600))?;
  kontur.seeded(&task, state);
  kontur.queue(&mut task, pipeline)?;
  Ok(task)
}

/// Stores `seeded` on `run`, written at `at`.
fn store(kontur: &mut Kontur, run: &Run, seeded: Seeded, at: Timestamp) -> Result<(), SeedError> {
  let invalid = |error: String| SeedError::Invalid(error);
  let (path, start, end, excerpt) = seeded.lines;
  let line = |number| NonZeroU32::new(number).ok_or_else(|| invalid("a line 0".to_owned()));
  let span = LineSpan::new(line(start)?, line(end)?).map_err(|error| invalid(error.to_string()))?;
  let anchor = LineAnchor::new(&path, DiffSide::New, span, excerpt).map_err(|error| invalid(error.to_string()))?;
  let messages = (0_i64..)
    .zip(seeded.messages)
    .map(|(after, (said, words))| {
      let stamp = Timestamp::from_unix_millis(at.unix_millis() + after * 60_000);
      ReviewMessage::restore(ReviewMessageId::generate(), said, words, stamp)
    })
    .collect();
  let parts = ThreadParts {
    id: ReviewThreadId::generate(),
    run: run.id(),
    kind: seeded.kind,
    anchor,
    head: seeded.head,
    created_at: at,
    resolved_at: seeded.resolved.then_some(at),
  };
  let thread = ReviewThread::new(parts, messages).map_err(|error| invalid(error.to_string()))?;
  kontur.db.add_review_thread(&thread)?;
  Ok(())
}

const fn not_sent() -> Said {
  Said::Comment(CommentState::NotSent { failure: None })
}

/// `run`'s latest checkpoint.
fn checkpoint(run: &Run) -> Result<Revision, SeedError> {
  run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().clone())
    .ok_or_else(|| SeedError::Invalid("the run committed nothing".to_owned()))
}

/// The content `work` gives the file at `path`.
fn content(work: &[(&str, String)], path: &str) -> Result<String, SeedError> {
  work
    .iter()
    .find(|(file, _)| *file == path)
    .map(|(_, content)| content.clone())
    .ok_or_else(|| SeedError::Invalid(format!("the work does not change {path}")))
}

/// The lines of `content` that the repository's first version of its file does not hold, by number from 1: the lines
/// the run's diff adds.
fn added(content: &str) -> Vec<(u32, String)> {
  let initial: Vec<String> = files::initial()
    .into_iter()
    .flat_map(|(_, text)| text.lines().map(str::to_owned).collect::<Vec<_>>())
    .collect();
  (1_u32..)
    .zip(content.lines())
    .filter(|(_, line)| !line.trim().is_empty() && !initial.iter().any(|known| known == line))
    .map(|(number, line)| (number, line.to_owned()))
    .collect()
}
