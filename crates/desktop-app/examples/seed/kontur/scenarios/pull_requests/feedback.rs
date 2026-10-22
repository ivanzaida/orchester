//! `pull-request-feedback`: `pull-requests` with agent feedback on #142 (`Pull request feedback` `seN1p`), whose
//! run is at work in Implement: a fix the Builder is working on, a second fix Orchester holds behind it, a question
//! it answered, a fix it committed that waits to be verified, a question on the whole pull request waiting in the
//! conversation, a question on code a later commit changed (outdated), and two resolved threads. #137's run
//! succeeded, so feedback on it starts a follow-up run; #134 has no task.

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::InvocationOutcome,
  conversation::{AgentMessage, Arrival, Message, MessageBody, Recipient},
  id::{FeedbackId, IdempotencyKey, ResourceId, RunId, StepExecutionId, TaskId},
  integration::WORKTREES_DIR,
  pull_request::feedback::{
    DiffSide, FeedbackAnchor, FeedbackKind, FeedbackPost, FeedbackThread, LineAnchor, LineSpan, ThreadParts,
    request_text,
  },
  resource::{ModelId, StepKey},
  revision::Revision,
  run::{Invocation, Run, StepExecutor, TokenUsage},
  store::{ActivityStore, ConversationStore, RunStore, TaskStore},
  time::Timestamp,
};

use super::{repo, revision};
use crate::kontur::{Kontur, SeedError, git};

/// The pull request the threads are on, and its task.
const NUMBER: u64 = 142;
const TITLE: &str = "Add retry to webhook sender";
/// The file whose diff the seed's #142 changes most, and where its added lines start.
const SENDER: &str = "src/webhooks/sender.rs";
const SENDER_FIRST: u32 = 11;
const JITTER: &str = "tests/webhooks/jitter.rs";

/// What happened to a thread's request.
#[derive(Clone, Copy)]
enum Fate {
  /// Orchester holds it.
  Held,
  /// In the conversation, not delivered yet.
  InConversation,
  /// Delivered to the turn at work.
  Working,
  /// Read in a turn that ended with this reply.
  Answered(&'static str),
  /// Read in a turn that committed with this subject and ended with the reply.
  Committed(&'static str, &'static str),
}

/// Lines of a file on the new side, and their code when it is not the seeded diff's.
#[derive(Clone, Copy)]
struct Lines {
  path: &'static str,
  start: u32,
  end: u32,
  code: Option<&'static [&'static str]>,
}

struct Seeded {
  kind: FeedbackKind,
  /// `None`: the whole pull request.
  lines: Option<Lines>,
  body: &'static str,
  minutes_ago: i64,
  fate: Fate,
  resolved_minutes_ago: Option<i64>,
  /// Written at a head before the pull request's.
  earlier: bool,
}

const fn lines(path: &'static str, start: u32, end: u32) -> Option<Lines> {
  Some(Lines {
    path,
    start,
    end,
    code: None,
  })
}

/// Oldest first, as the run's turns took them.
const THREADS: [Seeded; 8] = [
  Seeded {
    kind: FeedbackKind::Question,
    lines: Some(Lines {
      path: SENDER,
      start: 18,
      end: 19,
      code: Some(&["    let mut attempt = 0;", "    loop {"]),
    }),
    body: "Does this loop end when the endpoint is removed while it waits?",
    minutes_ago: 58,
    fate: Fate::Answered("It did not: the next commit replaced the loop with a bounded retry over retry_budget."),
    resolved_minutes_ago: None,
    earlier: true,
  },
  Seeded {
    kind: FeedbackKind::Question,
    lines: lines(SENDER, 30, 30),
    body: "Is retry_budget read once per delivery or on every attempt?",
    minutes_ago: 52,
    fate: Fate::Answered("Once per delivery: the settings are read before the first attempt and kept for the retries."),
    resolved_minutes_ago: Some(49),
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Fix,
    lines: None,
    body: "Add a test that runs migration 0042 on a database with deliveries in it.",
    minutes_ago: 46,
    fate: Fate::Answered("Added tests/webhooks/retry.rs::migrates_existing_deliveries; it passes."),
    resolved_minutes_ago: Some(41),
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Fix,
    lines: lines(JITTER, 9, 14),
    body: "Seed the jitter from a fixed RNG so this test gives the same delays on every run.",
    minutes_ago: 38,
    fate: Fate::Committed(
      "Seed retry jitter in tests",
      "Seeded the jitter with StdRng::seed_from_u64(7) in tests/webhooks/jitter.rs:9 and asserted the exact delays.",
    ),
    resolved_minutes_ago: None,
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Question,
    lines: lines(SENDER, 40, 40),
    body: "Why is the retry budget capped by MAX_RETRIES here and not by the endpoint's settings alone?",
    minutes_ago: 24,
    fate: Fate::Answered(
      "An endpoint's retry_budget comes from its settings and can be any number; src/webhooks/settings.rs:12 \
       makes MAX_RETRIES the ceiling so one endpoint cannot hold a worker for hours. I left the code as it is.",
    ),
    resolved_minutes_ago: None,
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Fix,
    lines: lines(SENDER, 43, 45),
    body: "Check for cancellation before each attempt, so a shutdown does not wait out the whole backoff.",
    minutes_ago: 9,
    fate: Fate::Working,
    resolved_minutes_ago: None,
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Fix,
    lines: lines(SENDER, 55, 56),
    body: "Log the attempt number with the delivery id when a retry is scheduled.",
    minutes_ago: 6,
    fate: Fate::Held,
    resolved_minutes_ago: None,
    earlier: false,
  },
  Seeded {
    kind: FeedbackKind::Question,
    lines: None,
    body: "Does this change the order in which deliveries reach an endpoint when one of them is retried?",
    minutes_ago: 3,
    fate: Fate::InConversation,
    resolved_minutes_ago: None,
    earlier: false,
  },
];

/// The run's agent at work in Implement, whose turns take the requests.
struct Builder {
  task: TaskId,
  run: RunId,
  execution: StepExecutionId,
  profile: ResourceId,
  model: ModelId,
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::seed(kontur)?;
  let builder = builder(kontur)?;
  end_open_turn(kontur, &builder, THREADS[0].minutes_ago + 1)?;
  for seeded in &THREADS {
    let thread = start(kontur, &builder, seeded)?;
    let post = &thread.posts()[0];
    if !matches!(seeded.fate, Fate::Held) {
      send(kontur, &builder, (&thread, post), seeded)?;
    }
    if let Some(minutes) = seeded.resolved_minutes_ago {
      kontur
        .db
        .resolve_feedback_thread(thread.id(), Some(kontur.ago(minutes)))?;
    }
  }
  Ok(())
}

fn builder(kontur: &Kontur) -> Result<Builder, SeedError> {
  let task = kontur
    .db
    .tasks()?
    .into_iter()
    .find(|task| task.details().title.as_str() == TITLE)
    .ok_or_else(|| SeedError::NoSuchTask(TITLE.to_owned()))?;
  let run = latest_run(kontur, task.id())?;
  let implement = StepKey::new("implement").map_err(invalid)?;
  let execution = run.latest_execution(&implement).ok_or(SeedError::NotAtWork)?.id();
  let Some(StepExecutor::Agent { profile, model, .. }) = run.snapshot().step(&implement).map(|step| step.executor())
  else {
    return Err(SeedError::NoSuchStep("implement".to_owned()));
  };
  Ok(Builder {
    task: task.id(),
    run: run.id(),
    execution,
    profile: profile.resource,
    model: model.clone(),
  })
}

fn latest_run(kontur: &Kontur, task: TaskId) -> Result<Run, SeedError> {
  Ok(
    kontur
      .db
      .runs_of_task(task)?
      .pop()
      .ok_or(SeedError::NotAtWork)?
      .map_err(|unreadable| unreadable.error)?,
  )
}

/// Ends the turn Implement started with `minutes` ago, so each request is read by a turn of its own.
fn end_open_turn(kontur: &mut Kontur, builder: &Builder, minutes: i64) -> Result<(), SeedError> {
  let at = kontur.ago(minutes);
  let open = kontur
    .db
    .invocations(builder.run)?
    .into_iter()
    .find(|invocation| invocation.execution == builder.execution && invocation.ended_at.is_none())
    .ok_or(SeedError::NotAtWork)?;
  if open.started_at >= at {
    return Err(SeedError::Invalid(format!(
      "Implement started after the first feedback thread, {minutes} minutes ago"
    )));
  }
  end_turn(kontur, open, at)
}

fn end_turn(kontur: &mut Kontur, mut invocation: Invocation, at: Timestamp) -> Result<(), SeedError> {
  invocation.ended_at = Some(at);
  invocation.usage = TokenUsage {
    input: 18_000,
    cached_input: 9_000,
    output: 2_400,
  };
  invocation.outcome = Some(InvocationOutcome::Completed);
  kontur.db.save_invocation(&invocation)?;
  Ok(())
}

fn start(kontur: &mut Kontur, builder: &Builder, seeded: &Seeded) -> Result<FeedbackThread, SeedError> {
  let at = kontur.ago(seeded.minutes_ago);
  let head = revision(if seeded.earlier { NUMBER + 1_000 } else { NUMBER })?;
  let body = MessageBody::new(seeded.body).map_err(invalid)?;
  let anchor = match seeded.lines {
    Some(lines) => FeedbackAnchor::Lines(anchor(lines)?),
    None => FeedbackAnchor::PullRequest,
  };
  let parts = ThreadParts {
    id: FeedbackId::generate(),
    task: builder.task,
    repo: repo()?,
    number: NUMBER,
    anchor,
    recipient: builder.profile,
    created_at: at,
    resolved_at: None,
  };
  let thread = FeedbackThread::new(parts, vec![FeedbackPost::new(seeded.kind, body, head, at)]).map_err(invalid)?;
  Ok(kontur.db.start_feedback_thread(IdempotencyKey::generate(), &thread)?)
}

fn anchor(lines: Lines) -> Result<LineAnchor, SeedError> {
  let code = match lines.code {
    Some(code) => code.iter().map(|line| (*line).to_owned()).collect(),
    None => (lines.start..=lines.end)
      .map(|number| seeded_line(lines.path, number))
      .collect(),
  };
  let start = NonZeroU32::new(lines.start).ok_or_else(|| SeedError::Invalid("line 0".to_owned()))?;
  let end = NonZeroU32::new(lines.end).ok_or_else(|| SeedError::Invalid("line 0".to_owned()))?;
  let span = LineSpan::new(start, end).map_err(invalid)?;
  LineAnchor::new(lines.path, DiffSide::New, span, code).map_err(invalid)
}

/// Line `number` of `path` as the seeded diff ([`super::diffs`]) adds it: every added line is the same retry,
/// numbered from the first.
fn seeded_line(path: &str, number: u32) -> String {
  let first = if path == SENDER { SENDER_FIRST } else { 1 };
  format!(
    "    let retry_{} = endpoint.settings.retry_budget.min(MAX_RETRIES);",
    number.saturating_sub(first)
  )
}

/// Sends `post` as the person's message to the Builder, and takes it as far as `seeded.fate` says.
fn send(
  kontur: &mut Kontur,
  builder: &Builder,
  (thread, post): (&FeedbackThread, &FeedbackPost),
  seeded: &Seeded,
) -> Result<(), SeedError> {
  let at = kontur.ago(seeded.minutes_ago);
  let body = MessageBody::new(&request_text(thread, post, None)).map_err(invalid)?;
  let mut message = Message::from_person(
    builder.task,
    Some(builder.run),
    Recipient::Agent(builder.profile),
    body,
    at,
  );
  kontur.db.send_feedback_post(post.id(), &message)?;
  if matches!(seeded.fate, Fate::Held | Fate::InConversation) {
    return Ok(());
  }
  let turn = kontur
    .db
    .start_invocation(builder.run, builder.execution, &builder.model, at)?;
  message
    .deliver(builder.run, (builder.execution, turn.id), at, Arrival::Prompt)
    .map_err(invalid)?;
  kontur.db.save_deliveries(std::slice::from_ref(&message))?;
  let (subject, reply) = match seeded.fate {
    Fate::Answered(reply) => (None, reply),
    Fate::Committed(subject, reply) => (Some(subject), reply),
    Fate::Held | Fate::InConversation | Fate::Working => return Ok(()),
  };
  if let Some(subject) = subject {
    commit(kontur, builder, subject, kontur.ago(seeded.minutes_ago - 2))?;
  }
  let answered = kontur.ago(seeded.minutes_ago - 3);
  message.acknowledge(answered).map_err(invalid)?;
  kontur.db.save_deliveries(std::slice::from_ref(&message))?;
  let reply = Message::from_agent(
    builder.task,
    builder.run,
    (builder.execution, turn.id),
    AgentMessage::Reply,
    MessageBody::new(reply).map_err(invalid)?,
    answered,
  );
  kontur.db.add_agent_message(&reply)?;
  end_turn(kontur, turn, answered)
}

/// Commits a change of the jitter test in the run's worktree with `subject`, and records it as a checkpoint.
fn commit(kontur: &mut Kontur, builder: &Builder, subject: &str, at: Timestamp) -> Result<(), SeedError> {
  let worktree = kontur.repo.join(WORKTREES_DIR).join(builder.run.to_string());
  let code = (1..=22)
    .map(|number| seeded_line(JITTER, number) + "\n")
    .collect::<String>();
  let seeded = format!("// let mut rng = StdRng::seed_from_u64(7);\n{code}");
  let committed: Revision = git::commit(&worktree, &[(JITTER, seeded)], subject)?;
  let mut run = latest_run(kontur, builder.task)?;
  let events = run.record_checkpoint(builder.execution, committed, at)?;
  kontur.db.append_events(&run, &events)?;
  Ok(())
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
