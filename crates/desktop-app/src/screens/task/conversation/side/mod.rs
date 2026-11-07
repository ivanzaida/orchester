//! The side question about a run (ORC-337, `Task page · redesign` 12–12d `j8r9z`, `gC8a7`, `Y595T`, `GVKQD`;
//! `docs/design/side-question-and-plan-mode.md`): a button in the run composer after *Attach* opens a pane beside the
//! transcript ([`pane`]), where the person asks about the run without writing to its agent. A separate read-only agent
//! answers on the Orchestrator's connection and model (`orchester_workspace::tasks::side_question`); the transcript
//! and the composer keep working beside it. *Send to {agent}* passes a question and its answer on as one ordinary
//! message; nothing else of the pane reaches the run's agent.
//!
//! What the pane keeps ([`SideQuestions`]) is the tab's for the run it shows: whether it is open, the draft and the
//! asks, stops and sends under way, which run on the app's runtime, so they end even when the pane is closed. The
//! questions and their answers are the task's, kept with it, and read again once one of those ended.

mod pane;
mod thread;

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, RadiusSize},
  },
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, SideQuestionId, TaskId},
  run::Answerer,
  time::Timestamp,
};
use orchester_workspace::{
  decisions::DecisionKind,
  messages::{self, Message},
  tasks::{
    conversation::follow::Sent,
    side_question::{self, Asking},
  },
  workspace::OpenProject,
};

pub(super) use self::pane::{PaneProps, pane};
use crate::{
  project::Projects,
  services::Services,
  theme::{ExtraColor, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The composer's *Side question* button: an `Icon button/Small` square (`b4mI1D`), as *Attach*'s.
const BUTTON_SIZE: f32 = 26.0;

/// A question to ask about a run of `project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AskRequest {
  project: OpenProject,
  asking: Asking,
}

impl lurq::app::component::DevtoolsInspectable for AskRequest {}

/// The side agent answering `question` in `project`, to stop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StopRequest {
  project: OpenProject,
  question: SideQuestionId,
}

impl lurq::app::component::DevtoolsInspectable for StopRequest {}

/// The answered side question `question` of `task` in `project`, to send on to the run's agent, pressed `by`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SendRequest {
  project: OpenProject,
  task: TaskId,
  question: SideQuestionId,
  by: Answerer,
}

impl lurq::app::component::DevtoolsInspectable for SendRequest {}

type AskAction = FutureAction<AskRequest, (), Message>;
type StopAction = FutureAction<StopRequest, (), Message>;
type SendAction = FutureAction<SendRequest, Sent, Message>;

/// A question asked now and not read back yet: shown as being answered meanwhile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pending {
  pub(super) id: SideQuestionId,
  pub(super) run: RunId,
  pub(super) text: String,
  pub(super) at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for Pending {}

/// The answer copied last, if one was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Copied(pub(super) Option<SideQuestionId>);

impl lurq::app::component::DevtoolsInspectable for Copied {}

/// What the pane keeps while the tab shows the run.
#[derive(Clone)]
pub(super) struct SideQuestions {
  pub(super) open: Signal<bool>,
  pub(super) draft: Signal<String>,
  /// Moves on once an ask, a stop or a send ended: the pane reads the task's questions again.
  pub(super) revision: Signal<u64>,
  pub(super) pending: Signal<Option<Pending>>,
  /// The answer copied last, which says *Copied*.
  pub(super) copied: Signal<Copied>,
  /// Where the thread is scrolled: it moves to its end for a new question and for its answer.
  pub(super) scroll: ScrollState,
  ask: AskAction,
  stop: StopAction,
  send: SendAction,
}

impl SideQuestions {
  /// Made once, in the tab's `create`, and provided to the composer and the pane.
  pub(super) fn provide(ctx: &mut Ctx) -> Self {
    let state = Self {
      open: ctx.signal(false),
      draft: ctx.signal(String::new()),
      revision: ctx.signal(0),
      pending: ctx.signal(None),
      copied: ctx.signal(Copied::default()),
      scroll: ScrollState::new(),
      ask: ask_action(ctx),
      stop: stop_action(ctx),
      send: send_action(ctx),
    };
    let (revision, pending, scroll) = (state.revision.clone(), state.pending.clone(), state.scroll.clone());
    ctx.watch(&state.ask.state(), move |result| {
      if !result.is_pending() {
        pending.set(None);
        revision.update(|at| *at += 1);
        scroll.scroll_to_bottom_pending();
      }
    });
    let revision = state.revision.clone();
    ctx.watch(&state.send.state(), move |result| {
      if !result.is_pending() {
        revision.update(|at| *at += 1);
      }
    });
    ctx.provide(state.clone());
    state
  }

  /// The tab's, when it shows a run.
  pub(super) fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context()
  }

  /// Asks `text` about `run` of `task` in `project`, unless it is blank or an answer is still coming.
  pub(super) fn ask(&self, project: &OpenProject, (task, run): (TaskId, RunId), text: &str) {
    if text.trim().is_empty() || self.asking() {
      return;
    }
    let id = SideQuestionId::generate();
    self.pending.set(Some(Pending {
      id,
      run,
      text: text.trim().to_owned(),
      at: Timestamp::now(),
    }));
    self.scroll.scroll_to_bottom_pending();
    self.ask.run(AskRequest {
      project: project.clone(),
      asking: Asking {
        id,
        task,
        run,
        text: text.to_owned(),
      },
    });
  }

  /// An answer is coming.
  pub(super) fn asking(&self) -> bool {
    self.ask.state().get_untracked().is_pending()
  }

  /// Stops the answer that is coming.
  pub(super) fn stop(&self, project: &OpenProject) {
    let Some(pending) = self.pending.get_untracked() else {
      return;
    };
    self.stop.run(StopRequest {
      project: project.clone(),
      question: pending.id,
    });
  }

  /// Sends the answer of `question` of `task` on to the run's agent, pressed `by`.
  pub(super) fn send(&self, project: &OpenProject, (task, question): (TaskId, SideQuestionId), by: Answerer) {
    if self.send.state().get_untracked().is_pending() {
      return;
    }
    self.send.run(SendRequest {
      project: project.clone(),
      task,
      question,
      by,
    });
  }

  /// A send is under way.
  pub(super) fn sending(&self) -> bool {
    self.send.state().get().is_pending()
  }

  /// Why the last ask, stop or send did not happen, if it did not.
  pub(super) fn refusal(&self) -> Option<Message> {
    [
      self.ask.state().get().error,
      self.stop.state().get().error,
      self.send.state().get().error,
    ]
    .into_iter()
    .flatten()
    .next()
  }
}

fn ask_action(ctx: &mut Ctx) -> AskAction {
  let services = Services::of(ctx);
  ctx.future_action(move |request: AskRequest| {
    let services = services.clone();
    let runtime = services.runtime.clone();
    async move {
      // On the runtime: the answer is kept once it comes, even when nothing shows the pane any more.
      let asking = async move {
        side_question::ask(
          (&services.runners, &services.programs),
          &request.project,
          request.asking,
        )
        .await
      };
      runtime.spawn(asking).await.map_err(|error| {
        tracing::error!(%error, "the side question did not finish");
        messages::interrupted()
      })?
    }
  })
}

fn stop_action(ctx: &mut Ctx) -> StopAction {
  let runners = Services::of(ctx).runners;
  ctx.future_action(move |request: StopRequest| {
    let runners = runners.clone();
    async move { side_question::stop(&runners, &request.project, request.question).await }
  })
}

fn send_action(ctx: &mut Ctx) -> SendAction {
  let services = Services::of(ctx);
  let projects = Projects::of(ctx);
  ctx.future_action(move |request: SendRequest| {
    let (services, projects) = (services.clone(), projects.clone());
    let runtime = services.runtime.clone();
    async move {
      let sending = async move {
        // Automation's send that would answer a waiting question needs the same consent as its answer.
        let workspace = services.workspace.clone();
        let allowed = move |kind: DecisionKind| kind.read_consent(&workspace);
        let sent = side_question::send_on(
          (&services.runners, &services.servers),
          (&services.workspace, &services.vault),
          &request.project,
          (request.task, request.question),
          (request.by, allowed),
        )
        .await
        .map(Sent::as_queued);
        // The conversation shows the message the send stored.
        projects.feed(request.project.project.id()).saved();
        sent
      };
      runtime.spawn(sending).await.map_err(|error| {
        tracing::error!(%error, "the side question's send did not finish");
        messages::interrupted()
      })?
    }
  })
}

/// The composer's *Side question* button for a run whose agent is `agent`: it opens the pane and closes it again,
/// pressed while it is open, its tooltip saying the agent does not see it. None while the tab shows no run.
pub(super) fn button(ctx: &mut Ctx, agent: &str) -> Option<Element> {
  let state = SideQuestions::of(ctx)?;
  let open = state.open.get();
  let label = ctx.t("side_question.button.label");
  let toggle = state.open;
  let mut square = pressable(move || toggle.update(|open| *open = !*open))
    .id("composer-side-question")
    .describe("state", if open { "pressed" } else { "enabled" })
    .describe("a11y_name", label)
    .size(Dimension::Px(BUTTON_SIZE), Dimension::Px(BUTTON_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()));
  if open {
    square = square.background(ExtraColor::Pressed.role());
  }
  let square = square.child(icon(
    Icon::MessageCircleQuestion,
    IconSize::Nav,
    PaletteColor::TextSecondary,
  ));
  let tip = ctx.t_args("side_question.button.tip", [("agent", agent)]);
  Some(Tooltip::new("composer-side-question", tip).wrap(ctx, square))
}
