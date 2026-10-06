//! The approval dialog ("Approval choices and waiting requests", `kpuQj`, `NV6xJ`, `XKja4`, `bjrgu`): one
//! waiting request at a time, the oldest first, with ‹ › to the others. The footer holds *Decide later*
//! (left), *Deny* and the primary *Approve once*, which has the focus; the wider answers are tiles above
//! it (*Approve similar in this run*, *Always allow…*) and, apart from them, *Approve everything in this
//! run*. *Always allow…* opens a second step that shows exactly what is added and where. Enter is
//! Approve once (in the second step its primary), Esc is Decide later (in the second step Back), never
//! Deny; a click on the backdrop does nothing. Tab follows the visual order and wraps in the dialog.
//!
//! The dialog answers through the runner; which answers a request offers is decided in core
//! (`crate::tasks::approvals::ApprovalChoices`). Decide later answers nothing: the request keeps waiting
//! in *Needs you*, on the task page and in the status bar ([`ApprovalQueue`]). A request whose agent can
//! no longer receive an answer leaves the dialog once the runner says so ([`ended`]).

mod always;
mod ended;
mod footer;
mod request;
mod scope;
mod subject_line;
mod wider;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Modal, ModalTarget},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::{RunId, TaskId},
  run::{Answerer, ApprovalAnswer, PendingToolApproval},
};
use orchester_ipc::{Command, Rejection};

pub(in crate::screens::project::task) use self::{
  ended::Unanswered,
  subject_line::{chip, quiet_reason},
  wider::{always_blocked, similar_unavailable},
};
use self::{ended::drop_when_ended, scope::ScopeState};
use super::{approval_arguments::ArgumentsCache, native_approval};
use crate::{
  messages::Message,
  runner::{RunRequestError, Runners},
  screens::project::approval_queue::{ApprovalQueue, QueuePlace},
  tasks::{approvals::ShownApproval, run::control_message},
  theme::ExtraColor,
  ui::{backdrop::backdrop, command_row::Copied, form::form, value_box::CopyState},
};

/// Dialog width (`gth5z` in `kpuQj`).
const DIALOG_WIDTH: f32 = 720.0;

#[derive(Clone, PartialEq)]
pub(super) struct ToolApprovalProps {
  shown: ShownApproval,
  place: QueuePlace,
  queue: ApprovalQueue,
  /// Every waiting request, which *Decide later* puts off together.
  waiting: Arc<Vec<String>>,
}

impl lurq::app::component::DevtoolsInspectable for ToolApprovalProps {}

/// The answer a button gives, for the label of the one under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pressed {
  Once,
  Similar,
  Always,
  Run,
  Deny,
}

impl lurq::app::component::DevtoolsInspectable for Pressed {}

/// An answer to one request, as the dialog and the Conversation's inline card give it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::project::task) struct Decision {
  pub(in crate::screens::project::task) run: RunId,
  pub(in crate::screens::project::task) approval: String,
  pub(in crate::screens::project::task) answer: ApprovalAnswer,
  /// The task whose agent asks, which says where a request that ended leads.
  pub(in crate::screens::project::task) task: Option<TaskId>,
}

impl lurq::app::component::DevtoolsInspectable for Decision {}

impl Decision {
  /// `answer` to `approval` of `task`, as the dialog and the timeline's inline card both give it.
  pub(in crate::screens::project::task) fn of(
    approval: &PendingToolApproval,
    task: Option<TaskId>,
    answer: ApprovalAnswer,
  ) -> Self {
    Self {
      run: approval.run,
      approval: approval.id.clone(),
      answer,
      task,
    }
  }
}

pub(in crate::screens::project::task) type DecisionAction = FutureAction<Decision, (), Unanswered>;

pub(in crate::screens::project::task) fn decision_action(ctx: &mut Ctx) -> DecisionAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |decision: Decision| {
    let runners = runners.clone();
    async move {
      runners
        .control_run(Command::DecideToolApproval {
          run: decision.run,
          approval: decision.approval.clone(),
          answer: decision.answer,
          by: Answerer::Person,
        })
        .await
        .map_err(|error| {
          tracing::warn!(%error, "the runner did not take the tool approval decision");
          match error {
            RunRequestError::Rejected(Rejection::ToolApprovalNotAwaited) => Unanswered::Ended {
              approval: decision.approval,
              task: decision.task,
            },
            RunRequestError::Rejected(Rejection::ToolApprovalAnswered) => Unanswered::Answered {
              approval: decision.approval,
            },
            error => Unanswered::Failed(control_message(&error)),
          }
        })
    }
  })
}

/// The dialog's layer: the modal over the window and its backdrop. Everything that changes while the
/// dialog is open lives in [`ToolApprovalPanel`] inside it, so the layer itself renders again only for
/// another request or step. lurq laid out a `Modal` at a window size from before the last resize when
/// the component that renders it rendered again on its own (seen with lurq 0.33.1).
pub(super) struct ToolApprovalDialog;

impl Component for ToolApprovalDialog {
  type Props = ToolApprovalProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let panel = ctx.mount::<ToolApprovalPanel>(props);
    // A click on the backdrop answers nothing and leaves the dialog open (*proposed*).
    Modal::new(backdrop(ctx, panel))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
  }
}

/// The dialog itself: the request, the answer being sent, the step's choices and its Copy buttons.
struct ToolApprovalPanel {
  pressed: Signal<Option<Pressed>>,
  copy: CopyState,
  arguments: ArgumentsCache,
  /// The prefix chosen in the *Always allow* step of a command, by its place among those offered.
  prefix: Signal<usize>,
  /// The scope of *Approve similar* is shown while its tile has the pointer or the focus.
  scope: ScopeState,
  /// The step's primary answer, which has the focus when the step opens.
  primary: ElementRef,
  /// Where the dialog is laid out, and the dialog, whose bottom stays put while the scope is open.
  stage: ElementRef,
  dialog: ElementRef,
}

impl Component for ToolApprovalPanel {
  type Props = ToolApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = decision_action(ctx);
    drop_when_ended(ctx, &action);
    let primary = ElementRef::new();
    ctx.focus(&primary);
    Self {
      pressed: ctx.signal(None),
      copy: CopyState {
        copied: ctx.signal(Copied::default()),
        hovered: ctx.signal(None),
      },
      arguments: ArgumentsCache::default(),
      prefix: ctx.signal(0),
      scope: ScopeState::new(ctx),
      primary,
      stage: ElementRef::new(),
      dialog: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let action = decision_action(ctx);
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    // Why the last answer did not go through, else that the request shown before ended. One that just
    // ended itself says nothing: it is leaving.
    let problem = match state.error.as_ref().filter(|_| !busy) {
      Some(Unanswered::Failed(message)) => Some(message.clone()),
      Some(Unanswered::Ended { .. } | Unanswered::Answered { .. }) => None,
      None => props
        .queue
        .shows_ended_notice()
        .then(|| Message::new("runs.toolApproval.endedNotice")),
    };
    let view = View {
      props: &props,
      busy,
      pressed: self.pressed.get(),
      error: problem.as_ref(),
      action: &action,
      pressed_signal: &self.pressed,
      copy: &self.copy,
      arguments: &self.arguments,
      prefix: &self.prefix,
      scope: &self.scope,
      primary: &self.primary,
    };
    let (sections, submit) = if props.place.always {
      always::sections(ctx, &view)
    } else {
      request::sections(ctx, &view)
    };
    // A native agent's command keeps the 640 of 04.H9 (`coKjY`).
    let width = match props.shown.approval.native {
      Some(_) => native_approval::DIALOG_WIDTH,
      None => DIALOG_WIDTH,
    };
    let panel = Column::new()
      .id("tool-approval-dialog")
      .ref_element(self.dialog.clone())
      .width(Dimension::Px(width))
      .max_height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .on_key_down(escape(&props))
      .with_children(sections);
    self.stage(form(submit, panel))
  }
}

impl ToolApprovalPanel {
  /// The dialog, centred. While the scope of *Approve similar* is open the dialog's bottom stays where
  /// it was, so it grows upward and the answers stay under the pointer; as tall as the window, it
  /// grows no more and its details scroll ([`ScopeState`]).
  fn stage(&self, dialog: Element) -> Element {
    let stage = Column::new()
      .ref_element(self.stage.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(dialog);
    if !self.scope.is_open() {
      return stage.justify(Justify::Center).into();
    }
    // As laid out last: with the scope closed, or open with this same room below.
    let (laid_out, shown) = (self.stage.bounds(), self.dialog.bounds());
    let below = (laid_out.y + laid_out.height - shown.y - shown.height).max(0.0);
    stage.justify(Justify::End).padding_bottom(Dimension::Px(below)).into()
  }
}

/// Esc: *Decide later* on the request, *Back* in the *Always allow* step.
fn escape(props: &ToolApprovalProps) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let queue = props.queue.clone();
  let waiting = props.waiting.clone();
  let always = props.place.always;
  move |event: KeyboardEvent| {
    if event.key != "Escape" {
      return;
    }
    event.prevent_default();
    if always {
      queue.always(None);
    } else {
      queue.defer(waiting.iter().cloned());
    }
  }
}

/// What the dialog's parts are built from.
struct View<'a> {
  props: &'a ToolApprovalProps,
  busy: bool,
  pressed: Option<Pressed>,
  error: Option<&'a Message>,
  action: &'a DecisionAction,
  pressed_signal: &'a Signal<Option<Pressed>>,
  copy: &'a CopyState,
  arguments: &'a ArgumentsCache,
  prefix: &'a Signal<usize>,
  scope: &'a ScopeState,
  primary: &'a ElementRef,
}

impl View<'_> {
  fn shown(&self) -> &ShownApproval {
    &self.props.shown
  }

  /// Gives `answer` to the request, unless an answer is under way.
  fn answer(&self, pressed: Pressed, answer: ApprovalAnswer) -> impl Fn() + Send + Sync + 'static {
    let action = self.action.clone();
    let requested = self.pressed_signal.clone();
    let queue = self.props.queue.clone();
    let decision = Decision::of(&self.shown().approval, self.shown().task_id, answer);
    move || {
      let state = action.state().get_untracked();
      if state.is_pending() || state.data.is_some() {
        return;
      }
      queue.clear_ended_notice();
      requested.set(Some(pressed));
      action.run(decision.clone());
    }
  }

  /// Whether `pressed` is the answer under way.
  fn under_way(&self, pressed: Pressed) -> bool {
    self.busy && self.pressed == Some(pressed)
  }
}

/// A command on one line, for where a request is named in passing (Needs you, the status bar): its first
/// line, marked when more follow.
pub(in crate::screens) fn command_line(ctx: &Ctx, shown: &ShownApproval) -> Arc<str> {
  match shown.first_line() {
    (line, true) => ctx.t_args("runs.toolApproval.firstLineMore", [("line", line)]),
    (line, false) => line.into(),
  }
}

/// The dialog of the request the queue shows among `pending`; nothing while it is closed.
pub(in crate::screens::project) fn modal(
  ctx: &mut Ctx,
  (pending, all): (&[ShownApproval], &[ShownApproval]),
  queue: &ApprovalQueue,
) -> Option<Element> {
  let mut place = queue.place(pending, all)?;
  let shown = pending.get(place.index)?.clone();
  // The Always allow step opens only while the request offers it.
  place.always &= shown.choices.always.is_ok();
  // Each step is mounted afresh, so its primary answer takes the focus when it opens.
  let step = if place.always { "always" } else { "request" };
  let key = format!("tool-approval-{}-{step}", shown.approval.id);
  Some(ctx.mount_keyed::<ToolApprovalDialog>(
    &key,
    ToolApprovalProps {
      shown,
      place,
      queue: queue.clone(),
      waiting: Arc::new(all.iter().map(|shown| shown.approval.id.clone()).collect()),
    },
  ))
}
