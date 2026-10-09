//! What the composer keeps while the task's page shows the task, whichever tab or run it shows: the
//! draft, the key a draft's words are stored under until they are, and the sends under way, so a tab
//! switch or a new run neither loses the draft nor stops a send. The task page makes it once
//! ([`ComposerState::new`]) and keeps it in each render ([`ComposerState::keep`]); the Conversation tab
//! and its messages reach it through the page's context.
//!
//! A send reads the task again and stores the message as its latest run's state offers then
//! (`outgoing`), then lets what its action says follow on the project's runner; the whole of it runs on
//! the app's runtime, so it ends even when no screen waits for it any more. A plan's changes are a
//! decision told to the runner. A message its agent's CLI did not take is queued again for when its
//! turn ends, and the runner of its own run is told.

use std::sync::Arc;

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
    outgoing::{Way, plan_changes, prepare_in, store_in},
    send_after_turn_in,
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskView},
  services::Services,
};

/// A message to send: for `task`'s latest run as the person saw it (`written_for`), sent `way`, read as
/// they read it (`lost`: the run lost its runner; `left_context`: they left a question's or a plan's mode).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) written_for: Option<RunId>,
  pub(super) reading: (bool, bool),
  pub(super) way: Way,
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

/// The composer's draft, its key, and its sends, kept by the task's page.
#[derive(Clone)]
pub struct ComposerState {
  pub(super) draft: Signal<String>,
  /// Set to put the focus in the message field.
  pub(super) focus: Signal<bool>,
  /// The words last sent and the key they are stored under until a store succeeds: sent again unchanged,
  /// they keep it, so a retry after an unclear failure stores nothing new; changed, they get a new one.
  attempt: Arc<Mutex<Option<(String, IdempotencyKey)>>>,
  pub(super) send: SendAction,
  pub(super) after_turn: AfterTurnAction,
}

impl ComposerState {
  /// The composer of the task `key` of `project`: made once by its page, before any of its own futures.
  pub fn new(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Self {
    let (send, after_turn) = (send_action(ctx), after_turn_action(ctx));
    let state = Self {
      draft: ctx.signal(String::new()),
      focus: ctx.signal(false),
      attempt: Arc::default(),
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
  }

  /// `text` was stored: the draft empties unless the person typed on meanwhile, and the next words get a
  /// key of their own.
  fn stored(&self, text: &str) {
    if self.draft.get_untracked() == text {
      self.draft.set(String::new());
    }
    let mut attempt = self.attempt.lock();
    if attempt.as_ref().is_some_and(|(sent, _)| sent == text) {
      *attempt = None;
    }
  }

  /// The key `text` is stored under: the one of the last attempt when the words did not change.
  pub(super) fn key_for(&self, text: &str) -> IdempotencyKey {
    let mut attempt = self.attempt.lock();
    match attempt.as_ref() {
      Some((sent, key)) if sent == text => *key,
      _ => {
        let key = IdempotencyKey::generate();
        *attempt = Some((text.to_owned(), key));
        key
      }
    }
  }

  /// Puts `text` into the composer, with the focus there; after what is written already, if anything is.
  pub(super) fn edit(&self, text: &str) {
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
    self.send.state().get_untracked().is_pending()
  }
}

fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: Request| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    async move {
      // On the runtime, not in the screen's future: a send under way ends even once nothing shows it.
      runtime.spawn(send(services, request)).await.map_err(|error| {
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
  let prepared = background::run(move || prepare_in(&root, (task, written_for), reading))
    .await?
    .map_err(|error| error.message())?;
  if let ComposerMode::PlanChanges { .. } = prepared.mode {
    let command = plan_changes(&prepared, &request.text, Answerer::Person).map_err(|error| error.message())?;
    services
      .runners
      .control_run_in(&request.project, command)
      .await
      .map_err(|error| {
        tracing::warn!(%error, "the runner did not take the plan's changes");
        orchester_workspace::tasks::run::control_message(&error)
      })?;
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
