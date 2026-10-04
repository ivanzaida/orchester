//! A step execution's header in the timeline (`mDnc3`): disclosure, step number, step and agent,
//! engine and time span (the invocation too while the step is open), tokens with those before the
//! first edit, and state: *Needs input* while its agent asks, *Waiting for approval* while a plan or
//! a tool request waits, *Approved · approved by you* once its plan was (`haIEI`, `ZxYGh`,
//! `AzKYQ`). A tool step names its command and how it exited, and has no model and *no tokens*. A
//! step that never ran after a failure is faded and reads *Not started* (`hep0l`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  run::{StepStatus, WaitReason},
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{super::heading::step_state, engine, first_edit, toggles::Toggles};
use crate::{
  display,
  tasks::timeline::StepHead,
  theme::{ExtraColor, ExtraSpacing, UNPARSED_OPACITY},
  ui::{
    command_row::command_preview,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Pressing the header opens or closes the step; pressing its tokens, when `tokens` says what to do
/// then, opens the Details pane on it.
pub(in crate::screens::project::task) fn step_header(
  ctx: &Ctx,
  (head, key, open): (&StepHead, &Arc<str>, bool),
  toggles: &Toggles,
  tokens_press: Option<&Arc<dyn Fn() + Send + Sync>>,
) -> Element {
  let ran = head.execution.is_some();
  let title = ctx.t_args(
    "runs.activity.stepTitle",
    [("step", &*head.step), ("agent", &*head.agent)],
  );
  let tokens = (head.command && ran).then(|| {
    Element::from(
      Text::new(&ctx.t("runs.activity.noTokens"))
        .variant(TypographyStyle::Mono)
        .nowrap(),
    )
  });
  let tokens = tokens.or_else(|| (!head.command && ran).then(|| agent_tokens(ctx, head, tokens_press)));
  let (glyph, state) = header_state(head);
  let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(chevron, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&format!("{:02}", head.number))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted),
    )
    // The step's name keeps its room in a narrow window; the engine and times give way first.
    .child(
      Text::new(&title)
        .id("activity-step-title")
        .variant(TypographyStyle::Label)
        .nowrap()
        .flex_shrink(0.0),
    )
    .with_children(ran.then(|| {
      Text::new(&header_meta(ctx, head, open))
        .id("activity-step-meta")
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex_shrink(1.0)
    }))
    .child(Spacer::new().flex(1.0))
    .with_children(tokens)
    .child(compact_status_label(glyph, &ctx.t(state)).id("activity-step-state"));
  let header = if ran {
    let (toggles, key) = (toggles.clone(), key.clone());
    pressable(surface(), move || toggles.flip(&key, open)).child(line)
  } else {
    surface().opacity(UNPARSED_OPACITY).child(line)
  };
  header
    .id("activity-step-header")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Group.role())
    .padding_bottom(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .describe(
      "execution",
      head
        .execution
        .map(|execution| execution.to_string())
        .unwrap_or_default(),
    )
    .describe("open", open.to_string())
    .into()
}

/// The tokens the agent used, with those before its first edit; pressed, `press` runs.
fn agent_tokens(ctx: &Ctx, head: &StepHead, press: Option<&Arc<dyn Fn() + Send + Sync>>) -> Element {
  let used = ctx.t_args(
    "runs.tokens",
    [("count", compact_tokens(head.usage.counted()).as_str())],
  );
  let used = first_edit::header_tokens(ctx, used, head.before_edit.as_ref(), head.ended.is_some());
  let text = Text::new(&used).variant(TypographyStyle::Mono).nowrap();
  match press {
    Some(press) => {
      let press = press.clone();
      pressable(surface().id("activity-step-tokens"), move || press())
        .child(text)
        .into()
    }
    None => text.into(),
  }
}

/// The state label: what the step waits for, *Approved* for an approved plan's step, else the
/// step's status.
fn header_state(head: &StepHead) -> (Icon, &'static str) {
  if head.execution.is_none() {
    return (Icon::CircleDashed, "runs.step.notStarted");
  }
  match head.status {
    Some(StepStatus::Waiting(WaitReason::Message)) => (Icon::MessageCircle, "runs.activity.state.needsInput"),
    Some(StepStatus::Waiting(WaitReason::Plan | WaitReason::Approval)) => {
      (Icon::Hand, "runs.activity.state.waitingApproval")
    }
    Some(StepStatus::Passed) if head.approved => (Icon::Check, "runs.activity.state.approved"),
    status => step_state(status),
  }
}

/// "Codex / Sol · 12:01–12:02", open "Codex / Sol · invocation 1 · 12:01–12:02", running "Codex /
/// Sol · invocation 1 · started 12:04"; "pnpm test · exit 0 · 12:22" for a tool step; and
/// "· approved by you" after a step whose plan the person approved.
fn header_meta(ctx: &Ctx, head: &StepHead, open: bool) -> Arc<str> {
  let meta = if head.command {
    tool_meta(ctx, head)
  } else {
    agent_meta(ctx, head, open)
  };
  if head.approved {
    return ctx.t_args("runs.activity.approvedByYou", [("meta", &*meta)]);
  }
  meta
}

fn agent_meta(ctx: &Ctx, head: &StepHead, open: bool) -> Arc<str> {
  let engine = engine(ctx, head.vendor.as_ref(), &head.model);
  // Orchester runs a native agent itself (04.H10 `rL1aI`).
  let engine = match head.provider {
    Some(provider) if provider.is_native() => ctx.t_args("runs.activity.engineNative", [("engine", &*engine)]),
    _ => engine,
  };
  let count = head.invocations.to_string();
  let start = head.started.map(display::clock).unwrap_or_default();
  let Some(ended) = head.ended else {
    return ctx.t_args(
      "runs.activity.headerRunning",
      [("engine", &*engine), ("count", &count), ("start", &start)],
    );
  };
  let time = span(ctx, head.started, ended);
  let key = match (open, head.invocations) {
    (true, _) => "runs.activity.headerOpen",
    (false, 1) => "runs.activity.headerFolded.one",
    (false, _) => "runs.activity.headerFolded.other",
  };
  ctx.t_args(key, [("engine", &*engine), ("count", &count), ("time", &*time)])
}

/// A tool step's command on one line and, unless it failed (its command's row says how), its exit
/// code; without a recorded command, that a command tool ran.
fn tool_meta(ctx: &Ctx, head: &StepHead) -> Arc<str> {
  let command: Arc<str> = match &head.tool_run {
    Some(run) => command_preview(&run.command).into(),
    None => ctx.t("runs.activity.commandTool"),
  };
  let start = head.started.map(display::clock).unwrap_or_default();
  let Some(ended) = head.ended else {
    return ctx.t_args(
      "runs.activity.commandHeaderRunning",
      [("command", &*command), ("start", &start)],
    );
  };
  let time = span(ctx, head.started, ended);
  let exit = head
    .tool_run
    .as_ref()
    .and_then(|run| run.exit_code)
    .filter(|_| head.status != Some(StepStatus::Failed));
  match exit {
    Some(code) => ctx.t_args(
      "runs.activity.commandHeaderExit",
      [("command", &*command), ("code", &code.to_string()), ("time", &*time)],
    ),
    None => ctx.t_args(
      "runs.activity.commandHeaderFinished",
      [("command", &*command), ("time", &*time)],
    ),
  }
}

/// "12:01–12:02", or one time when both fall on the same minute.
fn span(ctx: &Ctx, started: Option<Timestamp>, ended: Timestamp) -> Arc<str> {
  let end = display::clock(ended);
  match started.map(display::clock) {
    Some(start) if start != end => ctx.t_args(
      "runs.activity.timeSpan",
      [("start", start.as_str()), ("end", end.as_str())],
    ),
    _ => end.as_str().into(),
  }
}
