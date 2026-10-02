//! A one-request approval dialog. The oldest persisted request is shown first, and the next one
//! appears after the runner records a decision and notifies the task page.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, EngineVendor},
  id::RunId,
  resource::ModelId,
  run::PendingToolApproval,
};
use orchester_ipc::Command;

use super::{approval_arguments::ArgumentsCache, approval_value::ValueRows, native_approval};
use crate::{
  messages::Message,
  runner::Runners,
  tasks::{approvals::ShownApproval, run::control_message},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, select_menu_scrollbar},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    command_row::Copied,
    field::wrapping_field_error,
    form::form,
    icon::Icon,
    message_text,
    property::{ValueStyle, property, styled_property},
    value_box::CopyState,
  },
};

/// Dialog dimensions from the ORC-41 approval state (`ybPUT`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolApprovalProps {
  approval: PendingToolApproval,
  /// The CLI of the agent that asks, which decides what an approval means.
  provider: Option<AgentProvider>,
  /// Who receives the agent's prompts, as its engine is named.
  vendor: Option<EngineVendor>,
  /// The model the asking step runs on.
  model: Option<ModelId>,
  /// The asking step only reads: an approved command can still change files.
  read_only: bool,
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

/// The dialog's layer: the modal over the window and its backdrop. Everything that changes while the
/// dialog is open lives in [`ToolApprovalPanel`] inside it, so the layer itself renders again only
/// for another request. lurq lays out a `Modal` at a window size from before the last resize when
/// the component that renders it renders again on its own (seen with lurq 0.33.1).
pub(super) struct ToolApprovalDialog;

impl Component for ToolApprovalDialog {
  type Props = ToolApprovalProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let panel = ctx.mount::<ToolApprovalPanel>(props);
    let content = backdrop(ctx, form(None, panel));
    Modal::new(content).target(ModalTarget::Root).dismiss_on_escape(false)
  }
}

/// The dialog itself: the request, the decision being sent and the state of its Copy buttons.
struct ToolApprovalPanel {
  requested: Signal<Option<Choice>>,
  copy: CopyState,
  arguments: ArgumentsCache,
}

impl Component for ToolApprovalPanel {
  type Props = ToolApprovalProps;

  fn create(ctx: &mut Ctx) -> Self {
    decision_action(ctx);
    Self {
      requested: ctx.signal(None),
      copy: CopyState {
        copied: ctx.signal(Copied::default()),
        hovered: ctx.signal(None),
      },
      arguments: ArgumentsCache::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ToolApprovalProps>().clone();
    let action = decision_action(ctx);
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    let requested = self.requested.get();
    let error = state.error.filter(|_| !busy);
    dialog(
      ctx,
      &DialogState {
        props: &props,
        busy,
        requested,
        error: error.as_ref(),
        action: &action,
        requested_signal: &self.requested,
        copy: &self.copy,
        arguments: &self.arguments,
      },
    )
  }
}

pub(in crate::screens::project) fn modal(ctx: &mut Ctx, pending: &[ShownApproval]) -> Option<Element> {
  let shown = pending.first()?.clone();
  let key = format!("tool-approval-{}", shown.approval.id);
  Some(ctx.mount_keyed::<ToolApprovalDialog>(
    &key,
    ToolApprovalProps {
      approval: shown.approval,
      provider: shown.provider,
      vendor: shown.vendor,
      model: shown.model,
      read_only: shown.read_only,
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
  copy: &'a CopyState,
  arguments: &'a ArgumentsCache,
}

fn dialog(ctx: &mut Ctx, state: &DialogState<'_>) -> Element {
  let width = match state.props.approval.native {
    Some(_) => native_approval::DIALOG_WIDTH,
    None => DIALOG_WIDTH,
  };
  // No taller than the screen: the details scroll between the header and the buttons, so Approve
  // and Deny stay in view however long the values are.
  let details = ScrollVertical::new(details(&ValueRows::new(ctx, state.copy, state.arguments), state.props))
    .id("tool-approval-details-scroll")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .scrollbar(select_menu_scrollbar());
  Column::new()
    .id("tool-approval-dialog")
    .width(Dimension::Px(width))
    .max_height(Dimension::Pct(100.0))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header(ctx, state.props))
    .child(details)
    .child(actions(ctx, state))
    .into()
}

fn header(ctx: &Ctx, props: &ToolApprovalProps) -> Element {
  let approval = &props.approval;
  let command = approval.command;
  let title = ctx.t(if command {
    "runs.toolApproval.commandTitle"
  } else {
    "runs.toolApproval.title"
  });
  let explanation = if let Some(native) = approval.native {
    ctx.t_args(
      native_approval::explanation_key(native, props.read_only),
      [
        ("agent", approval.agent.as_str()),
        ("cwd", approval.target.as_deref().unwrap_or_default()),
      ],
    )
  } else if command {
    // Claude Code runs an approved command without a sandbox; Codex runs an approved escalation
    // outside its sandbox. Either can then change files, which a read-only step must say.
    let key = match (props.provider, props.read_only) {
      (Some(AgentProvider::ClaudeCodeCli), false) => "runs.toolApproval.commandExplanationClaude",
      (Some(AgentProvider::ClaudeCodeCli), true) => "runs.toolApproval.commandExplanationClaudeReadOnly",
      (Some(AgentProvider::CodexCli) | None, false) => "runs.toolApproval.commandExplanation",
      (Some(AgentProvider::CodexCli) | None, true) => "runs.toolApproval.commandExplanationReadOnly",
      // A native agent's command carries what it runs with; one without says only what it asks.
      (Some(AgentProvider::AnthropicApi | AgentProvider::OpenAiApi), _) => "runs.toolApproval.commandExplanationNative",
    };
    ctx.t_args(
      key,
      [
        ("agent", approval.agent.as_str()),
        ("cwd", approval.target.as_deref().unwrap_or_default()),
      ],
    )
  } else {
    ctx.t_args(
      "runs.toolApproval.explanation",
      [
        ("agent", approval.agent.as_str()),
        ("server", approval.server.as_str()),
        ("action", approval.action.as_str()),
      ],
    )
  };
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

fn details(values: &ValueRows<'_>, props: &ToolApprovalProps) -> Element {
  let ctx = values.ctx();
  let approval = &props.approval;
  let column = Column::new()
    .id("tool-approval-details")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role());
  let column = match approval.native {
    Some(native) => column.with_children(native_approval::details(
      values,
      approval,
      native,
      (props.vendor.as_ref(), props.model.as_ref()),
    )),
    None => cli_details(values, column, approval),
  };
  // A native request's scope line is the smaller 11/1.5 of `W30mF` in 04.H9.
  let scope = match approval.native {
    Some(_) => ExtraTypography::CellLabel.role(),
    None => TypographyStyle::Description,
  };
  column
    .child(Text::new(&ctx.t("runs.toolApproval.scope")).variant(scope))
    .with_children((props.remaining > 0).then(|| {
      Text::new(&ctx.t_args(
        "runs.toolApproval.remaining",
        [("count", props.remaining.to_string().as_str())],
      ))
      .variant(TypographyStyle::Description)
    }))
    .into()
}

/// A CLI's request: the agent, the MCP server of a tool action, the command or action, its target and
/// what the agent said about it. A command, action or target that does not fit beside its label
/// moves under it, whole. A command comes after its working directory, as the box it may need
/// closes the details (`Grcu7`); a CLI's request does not say which shell runs it.
fn cli_details(values: &ValueRows<'_>, column: Column, approval: &PendingToolApproval) -> Column {
  let ctx = values.ctx();
  let command = approval.command;
  let target_key = if command {
    "runs.toolApproval.cwd"
  } else {
    "runs.toolApproval.target"
  };
  let target = approval
    .target
    .as_deref()
    .map(|value| values.value(ValueRows::TARGET, &ctx.t(target_key), value));
  let column = column.child(property(&ctx.t("runs.toolApproval.agent"), &approval.agent));
  let column = if command {
    column.with_children(target).child(values.command(&approval.action))
  } else {
    column
      .child(styled_property(
        &ctx.t("runs.toolApproval.server"),
        &approval.server,
        ValueStyle::Mono,
      ))
      .child(values.value(ValueRows::ACTION, &ctx.t("runs.toolApproval.action"), &approval.action))
      .with_children(target)
  };
  column.with_children(
    approval
      .scope
      .as_deref()
      .map(|scope| scope_row(values, approval, scope)),
  )
}

/// What a request says beside its action: for a command, the reason the agent gave; for an MCP
/// call, its arguments as one line of JSON with secret values removed, recorded whole up to
/// `ARGUMENTS_CEILING` and shown here in full under their label.
fn scope_row(values: &ValueRows<'_>, approval: &PendingToolApproval, scope: &str) -> Element {
  let ctx = values.ctx();
  if approval.command {
    property(&ctx.t("runs.toolApproval.note"), scope)
  } else {
    values.arguments(&ctx.t("runs.toolApproval.arguments"), &approval.id, scope)
  }
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
    // Why the decision did not go through takes the room left of the buttons and wraps there: the
    // buttons keep their size and stay in the dialog however long the reason is.
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .with_children(
          state
            .error
            .map(|error| wrapping_field_error(&message_text(ctx, error))),
        ),
    )
    .child(
      ActionButton::new(deny_label)
        .id("tool-approval-deny")
        .icon(Icon::X)
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(deny)
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
        .on_click(approve)
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
