//! `orchestrator-events`: the `orchestrator` scenario with a conversation, the one the Orchestrator opens on, whose
//! tasks' runs had events of their own (ORC-755; `04 · Leading two modules` `OKtX2` and `Parts · Lines` e `c9YRv` in
//! `Orchestrator watch · redesign`): *Builder waits for a reply* on *Retry failed deliveries*, which the Orchestrator
//! answered, then *Merged into …* of *Shared rate limiter for deliveries*, which that run waits on, with the turn it
//! started; a run without progress for 20 minutes, a merge conflict, and a task the run waits on that was canceled,
//! which started no turn. The tasks are in the module *Webhooks v2*; the answers carry their *started by* tags.

use orchester_core::{
  id::ChatId,
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{ActionOutcome, AgentWait, Chat, ChatAction, TaskEvent, TaskEventKind, TurnState, WaitKind},
  task::organization::ChangeActor,
  time::Timestamp,
};

use super::{
  Part, keep, later,
  live::{self, Kept, Ran},
  set_up, turn, wait_turn,
};
use crate::kontur::{Kontur, SeedError, catalogue::Catalogue, shapes::Shape};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = set_up(kontur)?;
  let tasks = Tasks::seed(kontur, &catalogue)?;
  // After the `orchestrator` scenario's own conversations, so that this one is the latest.
  let start = kontur.ago(130);
  let chat = Chat::named(ChatId::generate(), "Webhooks v2 and Search index", start);
  turn(
    kontur,
    &chat,
    (
      "Answer agents from the brief and the docs. New tasks inside these modules are fine. Ask me before merging into \
       the module branch.",
      start,
    ),
    vec![Part::Reply(
      "I'll follow Webhooks v2 and answer its agents from the brief and the docs.",
    )],
    &TurnState::Answered,
  )?;
  let at = later(start, 30 * 60);
  answered_wait(kontur, (&chat, &tasks), at)?;
  merged(kontur, (&chat, &tasks), later(at, 77 * 60))?;
  let quiet = [
    (
      TaskEventKind::NoProgress { since: at, minutes: 20 },
      &tasks.retry,
      "stall",
    ),
    (TaskEventKind::MergeConflict, &tasks.signatures, "conflict"),
    (
      TaskEventKind::WaitedClosed {
        task: tasks.canceled.task.id(),
        run: Some(tasks.canceled.run.id()),
      },
      &tasks.retry,
      "closed",
    ),
  ];
  for (index, (kind, ran, name)) in (1_i64..).zip(quiet) {
    let at = later(at, (82 + index * 4) * 60);
    keep(kontur, &chat, &event(ran, (kind, at), name), at)?;
  }
  Ok(())
}

/// The tasks of the conversation, in *Webhooks v2*: the run that waits, the task it waits on (merged), one whose merge
/// conflicts and one canceled.
struct Tasks {
  retry: Ran,
  limiter: Ran,
  signatures: Ran,
  canceled: Ran,
}

impl Tasks {
  fn seed(kontur: &mut Kontur, catalogue: &Catalogue) -> Result<Self, SeedError> {
    let tasks = Self {
      retry: task(
        kontur,
        catalogue,
        (
          "Retry failed deliveries",
          "Retry a failed webhook delivery with backoff.",
        ),
        (Shape::Question("implement"), 5 * 60),
      )?,
      limiter: task(
        kontur,
        catalogue,
        (
          "Shared rate limiter for deliveries",
          "One rate limiter every delivery shares.",
        ),
        (Shape::Merged, 4 * 60),
      )?,
      signatures: task(
        kontur,
        catalogue,
        (
          "Sign every delivery attempt",
          "Sign each attempt again, not only the first.",
        ),
        (Shape::Succeeded, 3 * 60),
      )?,
      canceled: task(
        kontur,
        catalogue,
        ("Delivery receipts", "Keep a receipt of each delivery."),
        (Shape::Cancelled("implement"), 3 * 60),
      )?,
    };
    let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
    let name = Name::new("Webhooks v2").map_err(|error| invalid(&error))?;
    let description = "The second version of webhook deliveries.";
    let details =
      ModuleDetails::new(name, description, (None, None), ModuleState::Open).map_err(|error| invalid(&error))?;
    let at = kontur.ago(6 * 60);
    let module = kontur.db.create_module(details, at)?;
    for ran in [&tasks.retry, &tasks.limiter, &tasks.signatures, &tasks.canceled] {
      kontur
        .db
        .set_task_module(ran.task.id(), Some(module.id()), ChangeActor::Person, at)?;
    }
    Ok(tasks)
  }
}

/// Task `title` (`brief`) left in `shape`, its run started `minutes` ago.
fn task(
  kontur: &mut Kontur,
  catalogue: &Catalogue,
  (title, brief): (&str, &str),
  (shape, minutes): (Shape, i64),
) -> Result<Ran, SeedError> {
  let at = kontur.ago(minutes);
  live::task_in(kontur, catalogue, (title, brief), (shape, at))
}

/// The event `kind` of `ran`'s run at `at`, kept under a key of its own (`name`) that the follower never keeps.
fn event(ran: &Ran, (kind, at): (TaskEventKind, Timestamp), name: &str) -> Kept {
  let task = ran.task.key().to_string();
  Kept {
    wait: AgentWait {
      kind: WaitKind::Event(TaskEvent { kind, at }),
      title: ran.task.details().title.as_str().to_owned(),
      run: ran.run.id(),
      number: ran.run.number().get(),
      step: None,
      place: None,
      others: 0,
      agent: None,
      link: format!("/tasks/{task}"),
      task,
      automatic: true,
      continuing: None,
      merging: None,
    },
    key: format!("seed:event:{name}:{}", ran.run.id()),
  }
}

/// Builder's wait on *Retry failed deliveries* at `at`, which the Orchestrator answered.
fn answered_wait(kontur: &mut Kontur, (chat, tasks): (&Chat, &Tasks), at: Timestamp) -> Result<(), SeedError> {
  let mut asked = live::asked(kontur, &tasks.retry)?;
  asked.wait.automatic = true;
  let seq = keep(kontur, chat, &asked, at)?;
  let key = tasks.retry.task.key().to_string();
  wait_turn(
    kontur,
    (chat.id(), seq),
    later(at, 10),
    vec![
      Part::Reply(
        "The builder asked whether a retry keeps the first attempt's signature. docs/webhooks.md says every attempt is \
         signed again, so I answered that.",
      ),
      super::act(ChatAction::new("answer_agent", Some(key), ActionOutcome::Done)),
    ],
  )
}

/// *Shared rate limiter for deliveries*, which *Retry failed deliveries*' run waits on, merged at `at`, and the turn it
/// started.
fn merged(kontur: &mut Kontur, (chat, tasks): (&Chat, &Tasks), at: Timestamp) -> Result<(), SeedError> {
  let kind = TaskEventKind::WaitedMerged {
    task: tasks.limiter.task.id(),
    run: tasks.limiter.run.id(),
  };
  let kept = event(&tasks.retry, (kind, at), "merged");
  let seq = keep(kontur, chat, &kept, at)?;
  let key = tasks.retry.task.key().to_string();
  wait_turn(
    kontur,
    (chat.id(), seq),
    later(at, 10),
    vec![
      Part::Reply(
        "It waited on this. I brought the module branch into its worktree and told the builder the shared limiter is \
         there.",
      ),
      super::act(ChatAction::new("continue_run", Some(key), ActionOutcome::Done)),
    ],
  )
}
