//! A one-request approval dialog. The oldest persisted request is shown first, and the next one
//! appears after the runner records a decision and notifies the task page.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::RunId, run::PendingToolApproval};
use orchester_ipc::Command;

use crate::{
  messages::Message,
  runner::Runners,
  tasks::run::control_message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::wrapping_field_error,
    form::form,
    icon::Icon,
    message_text,
    property::{ValueStyle, property, styled_property},
  },
};

/// Dialog dimensions from the ORC-41 approval state (`ybPUT`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolApprovalProps {
  approval: PendingToolApproval,
  remaining: usize,
}

impl lurq::app::component::DevtoolsInspectable for ToolApprovalProps {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
  Approve,
  Deny,
}

impl lurq::app::component::DevtoolsInspectable for Choice {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Decision {
  run: RunId,
  approval: String,
  choice: Choice,
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
          approve: decision.choice == Choice::Approve,
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
  requested: Signal<Option<Choice>>,
}

impl Component for ToolApprovalDialog {
  type Props = ToolApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    decision_action(ctx);
    Self {
      requested: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let action = decision_action(ctx);
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    let requested = self.requested.get();
    let error = state.error.filter(|_| !busy);
    let content = dialog(
      ctx,
      &DialogState {
        props: &props,
        busy,
        requested,
        error: error.as_ref(),
        action: &action,
        requested_signal: &self.requested,
      },
    );
    Modal::new(content).target(ModalTarget::Root).dismiss_on_escape(false)
  }
}

pub(super) fn modal(ctx: &mut Ctx, pending: &[PendingToolApproval]) -> Option<Element> {
  let approval = pending.first()?.clone();
  let key = format!("tool-approval-{}", approval.id);
  Some(ctx.mount_keyed::<ToolApprovalDialog>(
    &key,
    ToolApprovalProps {
      approval,
      remaining: pending.len() - 1,
    },
  ))
}

struct DialogState<'a> {
  props: &'a ToolApprovalProps,
  busy: bool,
  requested: Option<Choice>,
  error: Option<&'a Message>,
  action: &'a DecisionAction,
  requested_signal: &'a Signal<Option<Choice>>,
}

fn dialog(ctx: &mut Ctx, state: &DialogState<'_>) -> Element {
  let panel = Column::new()
    .id("tool-approval-dialog")
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header(ctx, &state.props.approval))
    .child(details(ctx, state.props))
    .child(actions(ctx, state));
  backdrop(ctx, form(None, panel))
}

fn header(ctx: &Ctx, approval: &PendingToolApproval) -> Element {
  let title = ctx.t("runs.toolApproval.title");
  let explanation = ctx.t_args(
    "runs.toolApproval.explanation",
    [
      ("agent", approval.agent.as_str()),
      ("server", approval.server.as_str()),
      ("action", approval.action.as_str()),
    ],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&title).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(&explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}

fn details(ctx: &Ctx, props: &ToolApprovalProps) -> Element {
  let approval = &props.approval;
  Column::new()
    .id("tool-approval-details")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .child(property(&ctx.t("runs.toolApproval.agent"), &approval.agent))
    .child(styled_property(
      &ctx.t("runs.toolApproval.server"),
      &approval.server,
      ValueStyle::Mono,
    ))
    .child(styled_property(
      &ctx.t("runs.toolApproval.action"),
      &approval.action,
      ValueStyle::Mono,
    ))
    .with_children(
      approval
        .target
        .as_deref()
        .map(|target| styled_property(&ctx.t("runs.toolApproval.target"), target, ValueStyle::Mono)),
    )
    .with_children((props.remaining > 0).then(|| {
      Text::new(&ctx.t_args(
        "runs.toolApproval.remaining",
        [("count", props.remaining.to_string().as_str())],
      ))
      .variant(TypographyStyle::Description)
      .into()
    }))
    .into()
}

fn actions(ctx: &Ctx, state: &DialogState<'_>) -> Element {
  let deny_label = action_label(ctx, Choice::Deny, state.busy, state.requested);
  let approve_label = action_label(ctx, Choice::Approve, state.busy, state.requested);
  let deny = decision_press(state.props, Choice::Deny, state.action, state.requested_signal);
  let approve = decision_press(state.props, Choice::Approve, state.action, state.requested_signal);
  Row::new()
    .id("tool-approval-actions")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      state
        .error
        .map(|error| wrapping_field_error(&message_text(ctx, error)))
        .unwrap_or_else(|| Element::from(Spacer::new().flex(1.0))),
    )
    .child(
      ActionButton::new(deny_label)
        .id("tool-approval-deny")
        .icon(Icon::X)
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || deny())
        .enabled(!state.busy)
        .render(ctx),
    )
    .child(
      ActionButton::new(approve_label)
        .id("tool-approval-approve-once")
        .icon(Icon::Check)
        .primary()
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || approve())
        .enabled(!state.busy)
        .render(ctx),
    )
    .into()
}

fn action_label(ctx: &Ctx, choice: Choice, busy: bool, requested: Option<Choice>) -> Arc<str> {
  match (choice, busy && requested == Some(choice)) {
    (Choice::Approve, true) => ctx.t("runs.toolApproval.approving"),
    (Choice::Deny, true) => ctx.t("runs.toolApproval.denying"),
    (Choice::Approve, false) => ctx.t("runs.toolApproval.approveOnce"),
    (Choice::Deny, false) => ctx.t("runs.toolApproval.deny"),
  }
}

fn decision_press(
  props: &ToolApprovalProps,
  choice: Choice,
  action: &DecisionAction,
  requested: &Signal<Option<Choice>>,
) -> impl Fn() + Send + Sync + 'static {
  let action = action.clone();
  let requested = requested.clone();
  let decision = Decision {
    run: props.approval.run,
    approval: props.approval.id.clone(),
    choice,
  };
  move || {
    let state = action.state().get_untracked();
    if state.is_pending() || state.data.is_some() {
      return;
    }
    requested.set(Some(choice));
    action.run(decision.clone());
  }
}
