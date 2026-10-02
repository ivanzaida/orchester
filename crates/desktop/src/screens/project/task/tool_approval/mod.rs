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
//! in *Needs you*, on the task page and in the status bar ([`ApprovalQueue`]).

mod always;
mod footer;
mod request;
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
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::RunId, run::ApprovalAnswer};
use orchester_ipc::Command;

use super::{approval_arguments::ArgumentsCache, native_approval};
use crate::{
  messages::Message,
  runner::Runners,
  screens::project::approval_queue::{ApprovalQueue, QueuePlace},
  tasks::{approvals::ShownApproval, run::control_message},
  theme::ExtraColor,
  ui::{backdrop::backdrop, command_row::Copied, form::form},
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Decision {
  run: RunId,
  approval: String,
  answer: ApprovalAnswer,
}

impl lurq::app::component::DevtoolsInspectable for Decision {}

type DecisionAction = FutureAction<Decision, (), Message>;

fn decision_action(ctx: &mut Ctx) -> DecisionAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |decision: Decision| {
    let runners = runners.clone();
    async move {
      runners
        .control_run(Command::DecideToolApproval {
          run: decision.run,
          approval: decision.approval,
          answer: decision.answer,
        })
        .await
        .map_err(|error| {
          tracing::warn!(%error, "the runner did not take the tool approval decision");
          control_message(&error)
        })
    }
  })
}

pub(super) struct ToolApprovalDialog {
  pressed: Signal<Option<Pressed>>,
  copied: Signal<Copied>,
  arguments: ArgumentsCache,
  /// The prefix chosen in the *Always allow* step of a command, by its place among those offered.
  prefix: Signal<usize>,
  /// The scope of *Approve similar* is shown while its tile has the pointer or the focus.
  scope_open: Signal<bool>,
  /// The step's primary answer, which has the focus when the step opens.
  primary: ElementRef,
}

impl Component for ToolApprovalDialog {
  type Props = ToolApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    decision_action(ctx);
    let primary = ElementRef::new();
    ctx.focus(&primary);
    Self {
      pressed: ctx.signal(None),
      copied: ctx.signal(Copied::default()),
      arguments: ArgumentsCache::default(),
      prefix: ctx.signal(0),
      scope_open: ctx.signal(false),
      primary,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let action = decision_action(ctx);
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    let error = state.error.filter(|_| !busy);
    let view = View {
      props: &props,
      busy,
      pressed: self.pressed.get(),
      error: error.as_ref(),
      action: &action,
      pressed_signal: &self.pressed,
      copied: &self.copied,
      arguments: &self.arguments,
      prefix: &self.prefix,
      scope_open: &self.scope_open,
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
      .width(Dimension::Px(width))
      .max_height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .on_key_down(escape(&props))
      .with_children(sections);
    // A click on the backdrop answers nothing and leaves the dialog open (*proposed*).
    Modal::new(backdrop(ctx, form(submit, panel)))
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
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
  copied: &'a Signal<Copied>,
  arguments: &'a ArgumentsCache,
  prefix: &'a Signal<usize>,
  scope_open: &'a Signal<bool>,
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
    let decision = Decision {
      run: self.shown().approval.run,
      approval: self.shown().approval.id.clone(),
      answer,
    };
    move || {
      let state = action.state().get_untracked();
      if state.is_pending() || state.data.is_some() {
        return;
      }
      requested.set(Some(pressed));
      action.run(decision.clone());
    }
  }

  /// Whether `pressed` is the answer under way.
  fn under_way(&self, pressed: Pressed) -> bool {
    self.busy && self.pressed == Some(pressed)
  }
}

/// The dialog of the request the queue shows among `pending`; nothing while it is closed.
pub(in crate::screens::project) fn modal(
  ctx: &mut Ctx,
  pending: &[ShownApproval],
  queue: &ApprovalQueue,
) -> Option<Element> {
  let mut place = queue.place(pending)?;
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
      waiting: Arc::new(pending.iter().map(|shown| shown.approval.id.clone()).collect()),
    },
  ))
}
