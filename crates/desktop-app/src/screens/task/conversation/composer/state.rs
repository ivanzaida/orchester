//! What the composer keeps while the task's page shows the task, whichever tab or run it shows: the
//! context the person left and the sends under way, so a tab switch or a new run does not stop a send; the
//! draft, its key and the messages put back into it are kept for as long as the app runs ([`super::kept`]).
//! The task page makes it once ([`ComposerState::new`]) and keeps it in each render
//! ([`ComposerState::keep`]); the Conversation tab and its messages reach it through the page's context.
//!
//! A send reads the task again and does what the person pressed, if the run's state still offers it, or
//! is refused, keeping the draft (`outgoing`); it then lets what its action says follow on the project's
//! runner; the whole of it runs on
//! the app's runtime, so it ends even when no screen waits for it any more. A plan's changes are a
//! decision told to the runner. A message its agent's CLI did not take is queued again for when its
//! turn ends, and the runner of its own run is told.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  project::TaskKey,
  run::Answerer,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::conversation::{
    composer::ComposerMode,
    follow::{RunnerOf, Sent, Stored, follow, tell},
    outgoing::{Action, Context, Way, plan_changes, prepare_in, store_in},
    send_after_turn_in,
  },
  workspace::OpenProject,
};

use super::kept;
use crate::{
  navigation::Navigation,
  project::{ProjectFeed, ShownProject},
  routes::{ProjectPage, Route, TaskView},
  services::Services,
};

/// A message to send: for `task`'s latest run as the person saw it (`written_for`), sent `way` to do what
/// they pressed (`expected`), read as they read it (`lost`: the run lost its runner; `left`: the question's
/// or the plan's mode they left).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) written_for: Option<RunId>,
  pub(super) reading: (bool, Option<Context>),
  pub(super) way: Way,
  pub(super) expected: Action,
  pub(super) key: IdempotencyKey,
  pub(super) text: String,
}

impl lurq::app::component::DevtoolsInspectable for Request {}

/// What a send did, with the words it sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  /// The message `id` was stored in `mode`, and what followed it.
  Message {
    id: MessageId,
    sent: Sent,
    mode: &'static str,
    text: String,
  },
  /// The plan's changes were told to the runner.
  PlanChanges { text: String },
}

impl lurq::app::component::DevtoolsInspectable for Outcome {}

pub(super) type SendAction = FutureAction<Request, Outcome, Message>;

/// A message its agent's CLI did not take, to send after the turn of its own run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AfterTurn {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) run: RunId,
  pub(super) message: MessageId,
}

impl lurq::app::component::DevtoolsInspectable for AfterTurn {}

/// Why the message `message` was not queued again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Refused {
  pub(super) message: MessageId,
  pub(super) reason: Message,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

pub(super) type AfterTurnAction = FutureAction<AfterTurn, (MessageId, Sent), Refused>;

/// The refused messages put back into the composer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Handled(pub(super) Vec<MessageId>);

impl lurq::app::component::DevtoolsInspectable for Handled {}

/// The composer's draft, the context the person left, and its sends, kept by the task's page.
#[derive(Clone)]
pub struct ComposerState {
  task: TaskId,
  pub(super) draft: Signal<String>,
  /// Set to put the focus in the message field.
  pub(super) focus: Signal<bool>,
  /// The question's or the plan's mode the person left with its line's × or Esc: while the run's state
  /// offers it, the composer writes an ordinary message instead.
  pub(super) left: Signal<Option<Context>>,
  /// The refused messages put back into the composer: they offer nothing more.
  pub(super) handled: Signal<Handled>,
  /// The send an earlier page of the task started, under way when this one opened.
  flight: Option<kept::Flight>,
  /// That send has not ended yet.
  pub(super) in_flight: Signal<bool>,
  feed: ProjectFeed,
  pub(super) send: SendAction,
  pub(super) after_turn: AfterTurnAction,
}

impl ComposerState {
  /// The composer of `task`, the task `key` of `project`: made once by its page, before any of its own
  /// futures.
  pub fn new(ctx: &mut Ctx, project: &ShownProject, (task, key): (TaskId, &TaskKey)) -> Self {
    let (send, after_turn) = (send_action(ctx), after_turn_action(ctx));
    let flight = kept::flight(task);
    let state = Self {
      task,
      draft: ctx.signal(kept::draft(task)),
      focus: ctx.signal(false),
      left: ctx.signal(None),
      handled: ctx.signal(Handled(kept::handled(task))),
      in_flight: ctx.signal(flight.is_some()),
      flight,
      feed: project.feed.clone(),
      send,
      after_turn,
    };
    let navigation = Navigation::of(ctx);
    let summary = Route::Project(
      project.open.project.id(),
      ProjectPage::Task(key.clone(), TaskView::Summary),
    );
    let (feed, done) = (project.feed.clone(), state.clone());
    ctx.watch(&state.send.state(), move |result| {
      // A failure keeps the data of the send before it: only a send that went through counts.
      let Some(outcome) = result.data.as_ref().filter(|_| result.is_fulfilled()) else {
        return;
      };
      let (Outcome::Message { text, .. } | Outcome::PlanChanges { text }) = outcome;
      done.stored(text);
      feed.saved();
      // The message waits for the raised limit, which the person sets on the Summary.
      if let Outcome::Message {
        sent: Sent::AwaitsLimit,
        ..
      } = outcome
      {
        navigation.go(summary.clone());
      }
    });
    let handled = state.handled.clone();
    ctx.watch(&state.draft, move |text| {
      kept::set_draft(task, text);
      // Words put back and taken out again: their message offers again.
      let now = Handled(kept::handled(task));
      if handled.get_untracked() != now {
        handled.set(now);
      }
    });
    let feed = project.feed.clone();
    ctx.watch(&state.after_turn.state(), move |result| {
      if result.is_fulfilled() {
        feed.saved();
      }
    });
    state
  }

  /// Keeps the sends with the page: lurq keeps a component's futures by the order it asks for them in each
  /// render, so the page asks for these first, as it made them.
  pub fn keep(&self, ctx: &mut Ctx) {
    send_action(ctx);
    after_turn_action(ctx);
    self.await_flight(ctx);
  }

  /// While a send that an earlier page of the task started runs, the composer is busy; once it ends, the
  /// draft is the one kept by then (the send emptied it if it still held the sent words).
  fn await_flight(&self, ctx: &mut Ctx) {
    let Some(flight) = self.flight.clone() else {
      return;
    };
    let wait = ctx.future(flight.generation, move |_| {
      let mut ended = flight.ended.clone();
      async move {
        // A dropped sender ends the wait as well as its last word: either way the send is over.
        let _ = ended.wait_for(|ended| *ended).await;
        Ok::<(), Message>(())
      }
    });
    let waiting = !wait.state().get().is_fulfilled();
    if self.in_flight.get_untracked() == waiting {
      return;
    }
    self.in_flight.set(waiting);
    if !waiting {
      let kept = kept::draft(self.task);
      if self.draft.get_untracked() != kept {
        self.draft.set(kept);
      }
      self.feed.saved();
    }
  }

  /// `text` was stored: the draft empties unless the person typed on meanwhile.
  fn stored(&self, text: &str) {
    if self.draft.get_untracked() == text {
      self.draft.set(String::new());
    }
  }

  /// The key `text`, sent `how`, is stored under: the one of the last attempt when neither changed.
  pub(super) fn key_for(&self, text: &str, how: (Way, Action)) -> IdempotencyKey {
    kept::key_for(self.task, text, how)
  }

  /// Puts `text`, the refused message `message`, into the composer, with the focus there; after what is
  /// written already, if anything is. The message offers nothing more.
  pub(super) fn edit(&self, message: MessageId, text: &str) {
    kept::handle(self.task, message, text);
    self.handled.set(Handled(kept::handled(self.task)));
    let draft = self.draft.get_untracked();
    let text = if draft.trim().is_empty() {
      text.to_owned()
    } else {
      format!("{}\n\n{text}", draft.trim_end())
    };
    self.draft.set(text);
    self.focus.set(true);
  }

  /// A send is under way.
  pub(super) fn sending(&self) -> bool {
    self.send.state().get_untracked().is_pending() || self.in_flight.get_untracked()
  }
}

fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Request| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    async move {
      // On the runtime, not in the screen's future: a send under way ends even once nothing shows it, and a
      // page opened again meanwhile shows it busy.
      let task = request.task;
      let flight = kept::begin(task);
      let sending = async move {
        let sent = send(services, request).await;
        kept::end(task, flight);
        sent
      };
      runtime.spawn(sending).await.map_err(|error| {
        tracing::error!(%error, "the send did not finish");
        messages::interrupted()
      })?
    }
  })
}

/// Reads the task again, stores the message as its latest run's state offers now, and lets what follows
/// it follow; or tells the runner the plan's changes.
async fn send(services: Services, request: Request) -> Result<Outcome, Message> {
  let (root, task, written_for) = (request.project.repo_root.clone(), request.task, request.written_for);
  let reading = request.reading;
  let (way, expected) = (request.way, request.expected);
  let prepared = background::run(move || {
    let prepared = prepare_in(&root, (task, written_for), reading)?;
    // What the person pressed, or nothing: the run may have moved on since they saw it.
    prepared.check(way, expected)?;
    Ok(prepared)
  })
  .await?
  .map_err(|error: orchester_workspace::tasks::conversation::outgoing::OutgoingError| error.message())?;
  if let ComposerMode::PlanChanges { version, .. } = prepared.mode {
    let command =
      plan_changes(&prepared, &request.text, (Answerer::Person, Some(version))).map_err(|error| error.message())?;
    services
      .runners
      .control_run_in(&request.project, command)
      .await
      .map_err(|error| {
        tracing::warn!(%error, "the runner did not take the plan's changes");
        orchester_workspace::tasks::run::control_message(&error)
      })?;
    kept::stored(request.task, request.key, &request.text);
    return Ok(Outcome::PlanChanges { text: request.text });
  }
  let mode = super::mode_name(&prepared.mode);
  let (root, text, key, way) = (
    request.project.repo_root.clone(),
    request.text.clone(),
    request.key,
    request.way,
  );
  let (prepared, (then, stored)) = background::run(move || {
    let stored = store_in(&root, &prepared, (key, way), &text);
    (prepared, stored)
  })
  .await
  .and_then(|(prepared, stored)| {
    let stored = stored.map_err(|error| {
      tracing::warn!(?error, "the message was not stored");
      error.message()
    })?;
    Ok((prepared, stored))
  })?;
  let id = stored.id();
  kept::stored(request.task, key, &request.text);
  // Before the task has a run, its first step takes the message.
  let Some(run) = prepared.run() else {
    return Ok(Outcome::Message {
      id,
      sent: Sent::Taken,
      mode,
      text: request.text,
    });
  };
  let pipeline = prepared
    .detail
    .run
    .as_ref()
    .map(|run| run.pipeline.clone())
    .unwrap_or_default();
  let stored = Stored {
    root: &request.project.repo_root,
    task: request.task,
    run,
    key,
    pipeline: &pipeline,
  };
  let runner = (
    &services.runners,
    &services.servers,
    RunnerOf::Project(&request.project),
  );
  let sent = follow(runner, &stored, then).await;
  Ok(Outcome::Message {
    id,
    sent,
    mode,
    text: request.text,
  })
}

fn after_turn_action(ctx: &mut Ctx) -> AfterTurnAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: AfterTurn| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    let message = request.message;
    async move {
      runtime
        .spawn(send_after_turn(services, request))
        .await
        .map_err(|error| {
          tracing::error!(%error, "the send after the turn did not finish");
          Refused {
            message,
            reason: messages::interrupted(),
          }
        })?
    }
  })
}

/// Queues the message again for when its agent's turn ends, then tells the runner of its own run.
async fn send_after_turn(services: Services, request: AfterTurn) -> Result<(MessageId, Sent), Refused> {
  let refused = |reason: Message| Refused {
    message: request.message,
    reason,
  };
  let (root, task, message) = (request.project.repo_root.clone(), request.task, request.message);
  background::run(move || send_after_turn_in(&root, task, message))
    .await
    .map_err(refused)?
    .map_err(|error| {
      tracing::warn!(?error, "the message was not queued again");
      refused(error.message())
    })?;
  let runner = (
    &services.runners,
    &services.servers,
    RunnerOf::Project(&request.project),
  );
  let sent = tell(runner, (&request.project.repo_root, task, request.run)).await;
  Ok((message, sent))
}
