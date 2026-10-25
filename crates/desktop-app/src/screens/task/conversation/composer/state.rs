//! What the composer keeps while the task's page shows the task, whichever tab or run it shows: the
//! context the person left and the sends under way, so a tab switch or a new run does not stop a send; the
//! draft, its key and the messages put back into it are kept for as long as the app runs ([`super::kept`]).
//! The task page makes it once, in `create` ([`ComposerState::new`]); the Conversation tab and its messages
//! reach it through the page's context.
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
  attachment::NewAttachment,
  id::{IdempotencyKey, MessageId, RunId, TaskId},
  project::TaskKey,
  run::{Answerer, PlanVersion},
  sharing::TaskRevision,
};
use orchester_ipc::Command;
use orchester_workspace::{
  background,
  decisions::DecisionKind,
  messages::{self, Message},
  runner::AnswerMark,
  tasks::{
    answering::Answered,
    conversation::{
      composer::{ComposerMode, Then},
      follow::{RunnerOf, Sent, Stored, follow_seen, tell},
      outgoing::{Action, Context, Prepared, Way, plan_changes, prepare_in, store_answer_in},
      send_after_turn_as,
    },
  },
  workspace::OpenProject,
};

use super::kept;
use crate::{
  mcp::input_gate::InputGate,
  navigation::Navigation,
  project::{ProjectFeed, Projects, ShownProject},
  routes::{ProjectPage, Route, TaskView},
  services::Services,
  ui::attachments::draft::DraftAttachments,
};

/// A message to send: for `task`'s latest run as the person saw it (`written_for`), sent `way` to do what
/// they pressed (`expected`), read as they read it (`lost`: the run lost its runner; `left`: the question's
/// or the plan's mode they left).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Request {
  pub(super) project: OpenProject,
  pub(super) task: TaskId,
  pub(super) written_for: Option<RunId>,
  pub(super) revision: TaskRevision,
  pub(super) reading: (bool, Option<Context>),
  pub(super) way: Way,
  pub(super) expected: Action,
  pub(super) key: IdempotencyKey,
  pub(super) text: String,
  /// The files the person attached.
  pub(super) attachments: Vec<NewAttachment>,
  /// Captured before the event handler yields; None refuses a synthetic decision without consent.
  pub(super) by: Option<Answerer>,
}

impl lurq::app::component::DevtoolsInspectable for Request {}

/// What a send did, with the words and files it sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  /// The message `id` was stored in `mode`, and what followed it.
  Message {
    id: MessageId,
    sent: Sent,
    mode: &'static str,
    text: String,
    attachments: Vec<NewAttachment>,
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
  /// Who pressed, captured before the event handler yields: an MCP client's press is automation's.
  pub(super) by: Answerer,
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

/// *Stop turn* (ORC-92): the turn of the agent at work in run `run` of `project` stops; the run goes on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StopTurn {
  pub(super) project: OpenProject,
  pub(super) run: RunId,
}

impl lurq::app::component::DevtoolsInspectable for StopTurn {}

pub(super) type StopTurnAction = FutureAction<StopTurn, (), Message>;

/// The refused messages put back into the composer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Handled(pub(super) Vec<MessageId>);

impl lurq::app::component::DevtoolsInspectable for Handled {}

/// The composer's draft and its files, the context the person left, and its sends, kept by the task's page.
#[derive(Clone)]
pub struct ComposerState {
  task: TaskId,
  pub(super) draft: Signal<String>,
  /// The files the draft holds, and why the last ones given were not taken.
  pub(super) attached: DraftAttachments,
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
  pub(super) stop_turn: StopTurnAction,
  pub(super) gate: Option<InputGate>,
}

impl ComposerState {
  /// The composer of `task`, the task `key` of `project`: made once, in its page's `create`.
  pub fn new(ctx: &mut Ctx, project: &ShownProject, (task, key, kept): (TaskId, &TaskKey, bool)) -> Self {
    let (send, after_turn) = (send_action(ctx), after_turn_action(ctx));
    let flight = kept::flight(task);
    let state = Self {
      task,
      draft: ctx.signal(kept::draft(task)),
      attached: DraftAttachments::new(ctx, kept::attachments(task)),
      focus: ctx.signal(false),
      left: ctx.signal(None),
      handled: ctx.signal(Handled(kept::handled(task))),
      in_flight: ctx.signal(flight.is_some()),
      flight,
      feed: project.feed.clone(),
      send,
      after_turn,
      stop_turn: stop_turn_action(ctx),
      gate: ctx.use_context(),
    };
    let navigation = Navigation::of(ctx);
    let summary = Route::Project(
      project.open.project.id(),
      ProjectPage::task(key.clone(), TaskView::Summary, kept),
    );
    let (feed, done) = (project.feed.clone(), state.clone());
    ctx.watch(&state.send.state(), move |result| {
      // A failure keeps the data of the send before it: only a send that went through counts.
      let Some(outcome) = result.data.as_ref().filter(|_| result.is_fulfilled()) else {
        return;
      };
      match outcome {
        Outcome::Message { text, attachments, .. } => done.stored(text, attachments),
        Outcome::PlanChanges { text } => done.stored(text, &[]),
      }
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
    state.keep_drafts(ctx);
    let feed = project.feed.clone();
    ctx.watch(&state.after_turn.state(), move |result| {
      if result.is_fulfilled() {
        feed.saved();
      }
    });
    state
  }

  /// The draft and its files are kept beyond the page as they change.
  fn keep_drafts(&self, ctx: &mut Ctx) {
    let (task, handled) = (self.task, self.handled.clone());
    ctx.watch(&self.draft, move |text| {
      kept::set_draft(task, text);
      // Words put back and taken out again: their message offers again.
      let now = Handled(kept::handled(task));
      if handled.get_untracked() != now {
        handled.set(now);
      }
    });
    ctx.watch(&self.attached.attached, move |held| {
      kept::set_attachments(task, &held.0)
    });
  }

  /// While a send that an earlier page of the task started runs, the composer is busy; once it ends, the
  /// draft is the one kept by then (the send emptied it if it still held the sent words).
  pub fn await_flight(&self, ctx: &mut Ctx) {
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
      self.attached.replace(kept::attachments(self.task));
      self.feed.saved();
    }
  }

  /// `text` and `attachments` were stored: the draft empties unless the person typed on meanwhile, and loses the
  /// files sent.
  fn stored(&self, text: &str, attachments: &[NewAttachment]) {
    if self.draft.get_untracked() == text {
      self.draft.set(String::new());
    }
    self.attached.sent(attachments);
  }

  /// The key `text` and `attachments`, sent `how`, are stored under: the one of the last attempt when nothing
  /// changed.
  pub(super) fn key_for(&self, message: (&str, &[NewAttachment]), how: (Way, Action)) -> IdempotencyKey {
    kept::key_for(self.task, message, how)
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

  /// The person turns to the composer from a card, to answer a question or say what a plan needs: the
  /// question's or the plan's mode comes back if they had left it, and the focus goes into the box.
  pub fn take_up(&self) {
    if self.left.get_untracked().is_some() {
      self.left.set(None);
    }
    self.focus.set(true);
  }

  /// A send is under way.
  pub(super) fn sending(&self) -> bool {
    self.send.state().get_untracked().is_pending() || self.in_flight.get_untracked()
  }
}

fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  let projects = Projects::of(ctx);
  ctx.future_action(move |request: Request| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    let mark = answering(&services, &request);
    if mark.is_some() {
      projects.feed(request.project.project.id()).answer_sent();
    }
    async move {
      // On the runtime, not in the screen's future: a send under way ends even once nothing shows it, and a
      // page opened again meanwhile shows it busy.
      let task = request.task;
      let flight = kept::begin(task);
      let sending = async move {
        let sent = send(services, request).await;
        kept::end(task, flight);
        if let Some(mark) = mark {
          mark.settle(taken(&sent));
        }
        sent
      };
      runtime.spawn(sending).await.map_err(|error| {
        tracing::error!(%error, "the send did not finish");
        messages::interrupted()
      })?
    }
  })
}

/// Stores the message of `request` with its files as `prepared` offers, off the UI thread: the task as read and
/// what follows the stored message.
async fn store(
  request: &Request,
  prepared: Prepared,
  by: Answerer,
) -> Result<(Prepared, (Then, orchester_core::conversation::Message)), Message> {
  let (root, text, attachments, key, way) = (
    request.project.repo_root.clone(),
    request.text.clone(),
    request.attachments.clone(),
    request.key,
    request.way,
  );
  background::run(move || {
    let stored = store_answer_in(&root, &prepared, (key, way), (&text, &attachments), by);
    (prepared, stored)
  })
  .await
  .and_then(|(prepared, stored)| {
    let stored = stored.map_err(|error| {
      tracing::warn!(?error, "the message was not stored");
      error.message()
    })?;
    Ok((prepared, stored))
  })
}

/// Marks what `request` answers, if it answers what its run waits for (the agent's question, the plan's changes, a
/// continuation's decision) and may be sent: it leaves every *Needs you* as the person presses (`tasks::answering`).
fn answering(services: &Services, request: &Request) -> Option<AnswerMark> {
  request.by?;
  let answered = Answered::of_action(request.expected, request.written_for?)?;
  Some(services.runners.answering(&request.project.repo_root, answered))
}

/// Whether the runner took what a send answered: a message queued, or waiting for a raised limit, was not.
const fn taken(sent: &Result<Outcome, Message>) -> bool {
  matches!(
    sent,
    Ok(
      Outcome::PlanChanges { .. }
        | Outcome::Message {
          sent: Sent::Taken | Sent::Started(_),
          ..
        }
    )
  )
}
/// Reads the task again, stores the message as its latest run's state offers now, and lets what follows
/// it follow; or tells the runner the plan's changes.
async fn send(services: Services, request: Request) -> Result<Outcome, Message> {
  let by = request
    .by
    .ok_or_else(|| Message::new("appMcp.errors.automationNotAllowed"))?;
  let (root, task, written_for) = (request.project.repo_root.clone(), request.task, request.written_for);
  let reading = request.reading;
  let (way, expected) = (request.way, request.expected);
  let workspace = services.workspace.clone();
  let prepared = background::run(move || {
    let prepared = prepare_in(&root, (task, written_for), reading).map_err(|error| error.message())?;
    // What the person pressed, or nothing: the run may have moved on since they saw it.
    prepared.check(way, expected).map_err(|error| error.message())?;
    if let Some(kind) = prepared.decision_kind(way).filter(|_| by == Answerer::Automation) {
      let allowed = kind
        .read_consent(&workspace)
        .map_err(|error| messages::store_error(&error))?;
      if !allowed {
        return Err(Message::new("appMcp.errors.automationNotAllowed"));
      }
    }
    Ok(prepared)
  })
  .await??;
  if let ComposerMode::PlanChanges { version, .. } = prepared.mode {
    return send_plan_changes(&services, request, &prepared, (by, version)).await;
  }
  let mode = super::mode_name(&prepared.mode);
  let key = request.key;
  let (prepared, (then, stored)) = store(&request, prepared, by).await?;
  let id = stored.id();
  kept::stored(request.task, key, (&request.text, &request.attachments));
  // Before the task has a run, its first step takes the message.
  let Some(run) = prepared.run() else {
    return Ok(Outcome::Message {
      id,
      sent: Sent::Taken,
      mode,
      text: request.text,
      attachments: request.attachments,
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
  // On the task as the page showed it: Run again refuses one changed since.
  let sent = follow_seen(runner, &stored, (then, Some(request.revision))).await;
  if request.way == Way::StopTurn {
    stop_for_message(&services, &request.project, run).await;
  }
  Ok(Outcome::Message {
    id,
    sent,
    mode,
    text: request.text,
    attachments: request.attachments,
  })
}

/// Tells the runner the plan's changes, which are words only: a request with attached files is refused, keeping
/// them in the draft.
async fn send_plan_changes(
  services: &Services,
  request: Request,
  prepared: &Prepared,
  (by, version): (Answerer, PlanVersion),
) -> Result<Outcome, Message> {
  if !request.attachments.is_empty() {
    return Err(orchester_workspace::attachments::plan_changes_refused());
  }
  let command = plan_changes(prepared, &request.text, (by, Some(version))).map_err(|error| error.message())?;
  services
    .runners
    .control_run_in(&request.project, command)
    .await
    .map_err(|error| {
      tracing::warn!(%error, "the runner did not take the plan's changes");
      orchester_workspace::tasks::run::control_message(&error)
    })?;
  kept::stored(request.task, request.key, (&request.text, &[]));
  Ok(Outcome::PlanChanges { text: request.text })
}

/// *Stop turn and send*: once the message waits for the turn of the agent at work, that turn stops, so the agent reads
/// it as its next instruction. A turn that ended meanwhile was not stopped: the agent reads the message at its end
/// all the same.
async fn stop_for_message(services: &Services, project: &OpenProject, run: RunId) {
  if let Err(error) = services
    .runners
    .control_run_in(project, Command::StopTurn { run })
    .await
  {
    tracing::info!(%error, "the agent's turn was not stopped; it reads the message once its turn ends");
  }
}

fn stop_turn_action(ctx: &mut Ctx) -> StopTurnAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: StopTurn| {
    let services = services.clone();
    async move {
      services
        .runners
        .control_run_in(&request.project, Command::StopTurn { run: request.run })
        .await
        .map_err(|error| {
          tracing::warn!(%error, "the runner did not stop the agent's turn");
          orchester_workspace::tasks::run::control_message(&error)
        })
    }
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

/// Queues the message again for when its agent's turn ends, then tells the runner of its own run. Sent again
/// by automation while its agent waits on a question, it is that question's answer, under the same consent.
async fn send_after_turn(services: Services, request: AfterTurn) -> Result<(MessageId, Sent), Refused> {
  let refused = |reason: Message| Refused {
    message: request.message,
    reason,
  };
  let (root, task, message) = (request.project.repo_root.clone(), request.task, request.message);
  let workspace = services.workspace.clone();
  let allowed = move |kind: DecisionKind| kind.read_consent(&workspace);
  background::run(move || send_after_turn_as(&root, (task, message), (request.by, allowed)))
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
