//! `orchestrator-waits-on`: the `orchestrator` scenario with a conversation, the one the Orchestrator opens on, that
//! leads the module *Webhooks v2* and made runs wait on other tasks (ORC-784; Row 5 `lCgmQ` of `Orchestrator watch ·
//! redesign`): *Retry failed deliveries*' run waits on *Shared rate limiter for deliveries* (04 `OKtX2`, 09 `SLuUW`,
//! 10 `z5sQf`, `rlnEZ` a); one went on once the task it waited on was merged (b); one waits on a task that was
//! canceled (c, and its Summary f); one on a task closed without merging (d); and one whose wait the person dropped
//! (e). Each waiting run's agent asked a question and waits for the answer.

use orchester_core::{
  id::ChatId,
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{ActionOutcome, Chat, ChatAction, Subscription, TurnState, WaitsOnRow, WatchTarget},
  run::{BranchName, RunWaitsOn, WaitReason, WaitingRun},
  task::{Task, organization::ChangeActor},
  time::Timestamp,
};
use orchester_store::RunWaitEnd;

use super::{
  Part, act, later,
  live::{self, Ran},
  set_up, turn,
};
use crate::kontur::{Kontur, SeedError, catalogue::Catalogue, shapes::Shape};

/// The branch every waited task's work is to be merged into.
const BASE: &str = "module/webhooks-v2";

/// The order the conversation leads its module by.
const ORDER: &str = "Answer agents from the brief and the docs. New tasks inside these modules are fine. Ask me before merging into the \
   module branch.";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = set_up(kontur)?;
  let pairs = Pairs::seed(kontur, &catalogue)?;
  let start = kontur.ago(130);
  let chat = Chat::named(ChatId::generate(), "Webhooks v2 and Search index", start);
  turn(
    kontur,
    &chat,
    (ORDER, start),
    vec![Part::Reply("I'll lead Webhooks v2.")],
    &TurnState::Answered,
  )?;
  let target = WatchTarget::Module(pairs.module);
  kontur
    .db
    .start_subscription(&Subscription::new(chat.id(), target, ORDER, start))?;
  let at = kontur.ago(100);
  for pair in pairs.all() {
    wait(kontur, pair, at)?;
  }
  pairs.end(kontur)?;
  let row = |pair: &Pair| {
    act(ChatAction {
      waits_on: Some(WaitsOnRow {
        run: pair.waiting.run.id(),
        number: pair.waiting.run.number().get(),
        waited: pair.waited.key().to_string(),
      }),
      ..ChatAction::new(
        "wait_on_task",
        Some(pair.waiting.task.key().to_string()),
        ActionOutcome::Done,
      )
    })
  };
  let retry = pairs.retry.waiting.task.key().to_string();
  let limiter = pairs.retry.waited.key().to_string();
  turn(
    kontur,
    &chat,
    ("What do the delivery agents wait for?", later(at, -60)),
    vec![
      Part::Reply(
        "The builder asked whether a retry keeps the first attempt's signature. docs/webhooks.md says every attempt is \
         signed again, so I answered that. It also needs a shared rate limiter that nothing provides yet: I created it \
         and made the run wait on it.",
      ),
      act(ChatAction::new("answer_agent", Some(retry), ActionOutcome::Done)),
      act(ChatAction::new("create_task", Some(limiter), ActionOutcome::Done)),
      row(&pairs.retry),
      row(&pairs.continued),
      row(&pairs.canceled),
      row(&pairs.closed),
      row(&pairs.dropped),
    ],
    &TurnState::Answered,
  )
}

/// A run that waits on a task, and that task.
struct Pair {
  waiting: Ran,
  waited: Task,
}

/// The conversation's waits, one in each state, and the module their tasks are in.
struct Pairs {
  retry: Pair,
  continued: Pair,
  canceled: Pair,
  closed: Pair,
  dropped: Pair,
  module: orchester_core::id::ModuleId,
}

impl Pairs {
  fn seed(kontur: &mut Kontur, catalogue: &Catalogue) -> Result<Self, SeedError> {
    let continued = pair(
      (kontur, catalogue),
      (
        "Batch retries per endpoint",
        "Send an endpoint's retries in one batch.",
        9 * 60,
      ),
      (
        "Retry budget storage",
        "Keep each endpoint's retry budget.",
        Some(Shape::Merged),
      ),
    )?;
    let canceled = pair(
      (kontur, catalogue),
      ("Delivery dashboard", "Show deliveries per endpoint.", 8 * 60),
      (
        "Delivery receipts",
        "Keep a receipt of each delivery.",
        Some(Shape::Cancelled("implement")),
      ),
    )?;
    let closed = pair(
      (kontur, catalogue),
      (
        "Rotate signing keys",
        "Rotate the webhook signing keys monthly.",
        7 * 60,
      ),
      (
        "Key store",
        "Keep the signing keys in one store.",
        Some(Shape::Succeeded),
      ),
    )?;
    let dropped = pair(
      (kontur, catalogue),
      ("Endpoint health", "Mark an endpoint unhealthy after failures.", 6 * 60),
      ("Health probe", "Probe an endpoint before delivering.", None),
    )?;
    let retry = pair(
      (kontur, catalogue),
      (
        "Retry failed deliveries",
        "Retry a failed webhook delivery with backoff.",
        5 * 60,
      ),
      (
        "Shared rate limiter for deliveries",
        "One rate limiter every delivery shares.",
        None,
      ),
    )?;
    let module = module(kontur)?;
    let pairs = Self {
      retry,
      continued,
      canceled,
      closed,
      dropped,
      module,
    };
    let at = kontur.ago(10 * 60);
    for pair in pairs.all() {
      for task in [pair.waiting.task.id(), pair.waited.id()] {
        kontur
          .db
          .set_task_module(task, Some(pairs.module), ChangeActor::Person, at)?;
      }
    }
    Ok(pairs)
  }

  fn all(&self) -> [&Pair; 5] {
    [
      &self.retry,
      &self.continued,
      &self.canceled,
      &self.closed,
      &self.dropped,
    ]
  }

  /// Ends the waits that no longer wait, each as its state says.
  fn end(&self, kontur: &mut Kontur) -> Result<(), SeedError> {
    let ends = [
      (&self.continued, RunWaitEnd::Merged, 40),
      (&self.canceled, RunWaitEnd::Canceled, 30),
      (&self.closed, RunWaitEnd::ClosedUnmerged, 25),
    ];
    for (pair, end, minutes) in ends {
      let at = kontur.ago(minutes);
      kontur.db.end_run_wait(pair.waiting.run.id(), end, at)?;
    }
    let at = kontur.ago(20);
    kontur.db.drop_run_wait(self.dropped.waiting.run.id(), at)?;
    Ok(())
  }
}

/// A run on a task `waiting` (title, brief, started that many minutes ago) whose agent asked a question, and the task
/// `waited` it waits on: left in its shape, its run started half an hour before, or a draft without one.
fn pair(
  (kontur, catalogue): (&mut Kontur, &Catalogue),
  (title, brief, minutes): (&str, &str, i64),
  waited: (&str, &str, Option<Shape>),
) -> Result<Pair, SeedError> {
  let at = kontur.ago(minutes + 30);
  let waited = match waited {
    (name, about, Some(shape)) => live::task_in(kontur, catalogue, (name, about), (shape, at))?.task,
    (name, about, None) => live::draft(kontur, catalogue, (name, about), at)?,
  };
  let at = kontur.ago(minutes);
  let waiting = live::task_in(kontur, catalogue, (title, brief), (Shape::Question("implement"), at))?;
  Ok(Pair { waiting, waited })
}

/// The module *Webhooks v2*.
fn module(kontur: &mut Kontur) -> Result<orchester_core::id::ModuleId, SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let name = Name::new("Webhooks v2").map_err(|error| invalid(&error))?;
  let description = "The second version of webhook deliveries.";
  let details =
    ModuleDetails::new(name, description, (None, None), ModuleState::Open).map_err(|error| invalid(&error))?;
  let at = kontur.ago(11 * 60);
  Ok(kontur.db.create_module(details, at)?.id())
}

/// `pair`'s run waits on its task from `at`, until that task's work is merged into [`BASE`].
fn wait(kontur: &mut Kontur, pair: &Pair, at: Timestamp) -> Result<(), SeedError> {
  let invalid = |error: &dyn std::fmt::Display| SeedError::Invalid(error.to_string());
  let project = kontur.db.project().id();
  let waiting = WaitingRun {
    project,
    run: pair.waiting.run.id(),
    task: pair.waiting.task.id(),
    base: BranchName::new(BASE).map_err(|error| invalid(&error))?,
    waits_for: WaitReason::Message,
  };
  let wait = RunWaitsOn::new(waiting, pair.waited.id(), project).map_err(|error| invalid(&error))?;
  kontur.db.keep_run_wait(&wait, at)?;
  Ok(())
}
