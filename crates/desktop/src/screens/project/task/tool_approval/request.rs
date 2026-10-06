//! The request step of the approval dialog (`kpuQj`): the header with the request's place among the
//! waiting ones and ‹ ›, what the agent asks with every value whole (`bjrgu`), what Deny tells the
//! agent, the wider answers ([`super::wider`]) and the footer ([`super::footer`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::{connection::AgentProvider, permission::category::CommandCategory, run::PendingToolApproval};

use super::{
  super::{approval_value::ValueRows, native_approval, review_note::review_line},
  View, footer, scope, subject_line, wider,
};
use crate::{
  tasks::approvals::ShownApproval,
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, select_menu_scrollbar},
  ui::{
    form::Submit,
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop},
    property::property,
  },
};

/// The ‹ and › buttons beside the queue position (`YLfbU`, `H4ip59`): 26 square, the chevron in the
/// middle.
const QUEUE_BUTTON_SIZE: f32 = 26.0;

/// The request step's sections, and what Enter does: Approve once.
pub(super) fn sections(ctx: &mut Ctx, view: &View<'_>) -> (Vec<Element>, Submit) {
  // No taller than the screen: the details scroll between the header and the answers, so the wider
  // answers and the footer stay in view however long the values are.
  let details = ScrollVertical::new(details(&ValueRows::new(ctx, view.copy, view.arguments), view))
    .id("tool-approval-details-scroll")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .scrollbar(select_menu_scrollbar());
  let submit: Submit = (!view.busy).then(|| {
    Arc::new(view.answer(super::Pressed::Once, orchester_core::run::ApprovalAnswer::Once))
      as Arc<dyn Fn() + Send + Sync>
  });
  let sections = vec![
    header(ctx, view),
    details.into(),
    wider::answers(ctx, view),
    footer::request(ctx, view),
  ];
  (sections, submit)
}

fn header(ctx: &Ctx, view: &View<'_>) -> Element {
  let shown = view.shown();
  let title = ctx.t(if shown.approval.command {
    "runs.toolApproval.commandTitle"
  } else {
    "runs.toolApproval.title"
  });
  let place = &view.props.place;
  let position = ctx.t_args(
    "runs.toolApproval.queuePosition",
    [
      ("position", (place.index + 1).to_string().as_str()),
      ("count", place.count.to_string().as_str()),
    ],
  );
  let title_row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    // ‹ and › are as tall as the row and end at its right edge; their focus rings reach past it.
    .overflow_visible()
    .child(Text::new(&title).variant(ExtraTypography::DialogTitle.role()).flex(1.0))
    .child(
      Text::new(&position)
        .id("tool-approval-position")
        .variant(TypographyStyle::Caption),
    )
    .child(queue_button(
      view,
      Icon::ChevronLeft,
      "tool-approval-previous",
      place.previous.as_deref(),
    ))
    .child(queue_button(
      view,
      Icon::ChevronRight,
      "tool-approval-next",
      place.next.as_deref(),
    ));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(title_row)
    .child(Text::new(&explanation(ctx, shown)).variant(ExtraTypography::Explanation.role()))
    .with_children(
      shown
        .review
        .as_ref()
        .map(|review| review_line(ctx, "tool-approval-review".to_owned(), review)),
    )
    .into()
}

/// ‹ or › to the request `target`; at 0.4 and no Tab stop at either end of the queue.
fn queue_button(view: &View<'_>, glyph: Icon, id: &'static str, target: Option<&str>) -> Element {
  let button = Button::empty()
    .button()
    .id(id)
    .width(Dimension::Px(QUEUE_BUTTON_SIZE))
    .height(Dimension::Px(QUEUE_BUTTON_SIZE))
    // A lurq button lays its children out as a row from the start; the glyph is centred both ways.
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Marker, PaletteColor::TextPrimary));
  match target {
    Some(target) => {
      let (queue, target) = (view.props.queue.clone(), target.to_owned());
      keyboard_stop(button, Ring::Outer)
        .cursor(CursorIcon::Pointer)
        .describe("state", "enabled")
        .on_click(move |_| queue.review(&target))
        .into()
    }
    None => button
      .focusable(false)
      .opacity(DISABLED_OPACITY)
      .describe("state", "unavailable")
      .into(),
  }
}

fn explanation(ctx: &Ctx, shown: &ShownApproval) -> Arc<str> {
  let approval = &shown.approval;
  let names = [("agent", approval.agent.as_str()), ("server", approval.server.as_str())];
  if let Some(native) = approval.native {
    return ctx.t_args(native_approval::explanation_key(native, shown.read_only), names);
  }
  if !approval.command {
    return ctx.t_args("runs.toolApproval.explanation", names);
  }
  // Claude Code runs an approved command without a sandbox; Codex runs an approved escalation
  // outside its sandbox. Either can then change files, which a read-only step must say; otherwise the
  // explanation says what the policy asks about (`xNcLS` in `NV6xJ`, `V8j3QO` in `bjrgu`).
  let key = match (shown.provider, shown.read_only) {
    (Some(AgentProvider::ClaudeCodeCli), true) => "runs.toolApproval.commandExplanationClaudeReadOnly",
    (Some(AgentProvider::CodexCli) | None, true) => "runs.toolApproval.commandExplanationReadOnly",
    // A native agent's command carries what it runs with; one without says only what it asks.
    (Some(AgentProvider::AnthropicApi | AgentProvider::OpenAiApi), _) => "runs.toolApproval.commandExplanationNative",
    (_, false) => policy_explanation(shown),
  };
  ctx.t_args(key, names)
}

/// What the asking policy asks about, by the command's category (or that it reads credentials).
const fn policy_explanation(shown: &ShownApproval) -> &'static str {
  if shown.choices.secrets {
    return "runs.toolApproval.policyExplanation.credentials";
  }
  match shown.choices.category {
    Some(CommandCategory::ReadOnly) => "runs.toolApproval.policyExplanation.readOnly",
    Some(CommandCategory::Worktree) => "runs.toolApproval.policyExplanation.worktree",
    Some(CommandCategory::Network) => "runs.toolApproval.policyExplanation.network",
    Some(CommandCategory::OutsideWrite) => "runs.toolApproval.policyExplanation.outsideWrite",
    Some(CommandCategory::HistoryDestructive) => "runs.toolApproval.policyExplanation.historyDestructive",
    Some(CommandCategory::Unknown) | None => "runs.toolApproval.policyExplanation.unknown",
  }
}

fn details(values: &ValueRows<'_>, view: &View<'_>) -> Element {
  let shown = view.shown();
  let approval = &shown.approval;
  let column = Column::new()
    .id("tool-approval-details")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role());
  let column = column.child(subject_line::line(values.ctx(), shown));
  let column = match approval.native {
    Some(native) => column.with_children(native_approval::details(
      values,
      approval,
      native,
      (shown.vendor.as_ref(), shown.model.as_ref()),
    )),
    None => cli_details(values, column, shown),
  };
  column
    .with_children(
      view
        .scope_open
        .get()
        .then(|| scope::panel(values.ctx(), shown))
        .flatten(),
    )
    .child(denial(values.ctx(), shown))
    .into()
}

/// A CLI's request under its subject line: the agent, then a command's working directory, what the agent
/// said about it and the command, or a tool action and its target and arguments (`kpuQj`, `NV6xJ`). A
/// command, action or target that does not fit beside its label moves under it, whole. A command comes
/// last, as the box it may need closes the details (`Grcu7`), labelled with why it asks; a CLI's
/// request does not say which shell runs it.
fn cli_details(values: &ValueRows<'_>, column: Column, shown: &ShownApproval) -> Column {
  let ctx = values.ctx();
  let approval = &shown.approval;
  let command = approval.command;
  let target_key = if command {
    "runs.toolApproval.cwd"
  } else {
    "runs.toolApproval.target"
  };
  // A command's folder as the repository names it (`.orchester/worktrees/…`), an MCP target as recorded.
  let target = if command {
    shown.cwd.as_deref()
  } else {
    approval.target.as_deref()
  };
  let target = target.map(|value| values.value(ValueRows::TARGET, &ctx.t(target_key), value));
  let column = column.child(property(&ctx.t("runs.toolApproval.agent"), &agent(ctx, shown)));
  let scope = approval
    .scope
    .as_deref()
    .map(|scope| scope_row(values, approval, scope));
  if !command {
    return column
      .child(values.value(ValueRows::ACTION, &ctx.t("runs.toolApproval.action"), &approval.action))
      .with_children(target)
      .with_children(scope);
  }
  let label = shown
    .choices
    .reason
    .as_ref()
    .and_then(|reason| subject_line::command_label(ctx, reason));
  let command = match label {
    Some(label) => values.labelled_command(&label, &approval.action),
    None => values.command(&approval.action),
  };
  column.with_children(target).with_children(scope).child(command)
}

/// The agent, with its step, run and task when they can be read (`dcjnZ`).
pub(super) fn agent(ctx: &Ctx, shown: &ShownApproval) -> String {
  match (&shown.step, shown.run, &shown.task) {
    (Some(step), Some(run), Some(task)) => ctx
      .t_args(
        "runs.toolApproval.agentValue",
        [
          ("agent", shown.approval.agent.as_str()),
          ("step", step),
          ("run", run.get().to_string().as_str()),
          ("task", task),
        ],
      )
      .to_string(),
    _ => shown.approval.agent.clone(),
  }
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

/// What Deny tells the agent, word for word (`Z5Rea`): Orchester's message for Claude Code; Codex
/// declines and tells it in its own words.
fn denial(ctx: &Ctx, shown: &ShownApproval) -> Element {
  let agent = shown.approval.agent.as_str();
  let key = if shown.choices.codex {
    "runs.toolApproval.denyTellsCodex"
  } else {
    "runs.toolApproval.denyTells"
  };
  let text = ctx.t_args(key, [("agent", agent), ("message", &*shown.choices.told)]);
  Row::new()
    .id("tool-approval-deny-tells")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::MessageSquareX, IconSize::Small, PaletteColor::TextMuted))
    .child(Text::new(&text).variant(ExtraTypography::CellLabel.role()).flex(1.0))
    .into()
}
