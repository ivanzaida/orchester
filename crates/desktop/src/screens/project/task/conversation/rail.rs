//! The step rail at the right of the chat (`E2qRnp` in `kpnmP`): 248 wide on `$sidebar` with a
//! hairline at its left. Its head says *Steps* and how many, and collapses it to the strip
//! ([`super::strip`]). *All steps* comes first, then a row per step of the run's pipeline: its state
//! icon, number, name and one meta line (agent · tokens · time, or what it waits for). The row picked
//! is `$hover`; picking one shows its step only. A step that has not started is at 0.7 and cannot be
//! picked. During a search each row says its matches (`Hn1xv`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{run::WaitReason, tokens::compact_tokens};

use super::StepFilter;
use crate::{
  display,
  tasks::timeline::rail::{RailState, RailStep, RailSummary},
  theme::{CONVERSATION_RAIL_COLLAPSE_BELOW_WINDOW, ExtraColor, ExtraSpacing, ExtraTypography, NOT_STARTED_OPACITY},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
  },
};

/// The rail's width and its head's height (`E2qRnp`, `JDaBJ`).
pub(super) const RAIL_WIDTH: f32 = 248.0;
pub(super) const HEAD_HEIGHT: f32 = 52.0;

/// Whether the rail is collapsed to its strip and, while it is, open over the chat.
#[derive(Clone)]
pub(super) struct RailToggle {
  /// What the person chose in this tab; `None` until they choose, when the window's width decides.
  pub(super) collapsed: Signal<Option<bool>>,
  /// The full rail shows over the chat while collapsed (`I35q7D`).
  pub(super) over: Signal<bool>,
}

impl RailToggle {
  /// Collapsed by hand, or by default in a window narrower than 1200.
  pub(super) fn is_collapsed(&self, window: f32) -> bool {
    self
      .collapsed
      .get()
      .unwrap_or(window < CONVERSATION_RAIL_COLLAPSE_BELOW_WINDOW)
  }

  /// Collapses the rail to its strip, or closes it over the chat.
  fn collapse(&self) -> impl Fn() + Send + Sync + 'static {
    let (collapsed, over) = (self.collapsed.clone(), self.over.clone());
    move || {
      if over.get_untracked() {
        over.set(false);
      } else {
        collapsed.set(Some(true));
      }
    }
  }

  /// Opens the rail from its strip: in place in a wide window, over the chat in a narrow one.
  pub(super) fn expand(&self, window: f32) -> impl Fn() + Send + Sync + 'static {
    let (collapsed, over) = (self.collapsed.clone(), self.over.clone());
    move || {
      if window < CONVERSATION_RAIL_COLLAPSE_BELOW_WINDOW {
        over.set(true);
      } else {
        collapsed.set(Some(false));
      }
    }
  }
}

/// What the rail shows and changes.
pub(super) struct RailProps<'a> {
  pub(super) steps: &'a [RailStep],
  pub(super) summary: RailSummary,
  /// While searching: the matches of the run, and of each step in `steps`' order.
  pub(super) matches: Option<(usize, Vec<usize>)>,
  pub(super) filter: &'a Signal<StepFilter>,
  pub(super) toggle: &'a RailToggle,
  pub(super) picker: &'a Picker,
}

/// Picks a step, or all steps (`None`), in the rail or its strip.
pub(super) type Picker = Arc<dyn Fn(StepFilter) + Send + Sync>;

impl RailProps<'_> {
  fn pick(&self, key: StepFilter) -> impl Fn() + Send + Sync + 'static {
    let picker = self.picker.clone();
    move || picker(key.clone())
  }
}

/// The full rail.
pub(super) fn rail(ctx: &Ctx, props: &RailProps<'_>) -> Element {
  let picked = props.filter.get().0;
  let all = all_row(ctx, props, picked.is_none());
  let rows = props.steps.iter().enumerate().map(|(index, step)| {
    let matches = props.matches.as_ref().and_then(|(_, steps)| steps.get(index).copied());
    step_row(ctx, props, step, (picked.as_ref() == Some(&step.key), matches))
  });
  let list = Column::new()
    .id("conversation-rail-list")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Hair.role())
    .child(all)
    .with_children(rows);
  Column::new()
    .id("conversation-rail")
    .width(Dimension::Px(RAIL_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(head(ctx, props))
    .child(ScrollVertical::new(list).width(Dimension::Pct(100.0)).flex(1.0))
    .into()
}

/// *Steps*, how many, and the control that collapses the rail.
fn head(ctx: &Ctx, props: &RailProps<'_>) -> Element {
  let collapse = pressable(surface().id("conversation-rail-collapse"), props.toggle.collapse()).child(icon(
    Icon::PanelRightClose,
    IconSize::Event,
    PaletteColor::TextSecondary,
  ));
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t("runs.conversation.rail.steps"))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&props.steps.len().to_string())
        .variant(ExtraTypography::NavCount.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(collapse)
    .into()
}

/// *All steps* with the run's executions and tokens, or its matches.
fn all_row(ctx: &Ctx, props: &RailProps<'_>, picked: bool) -> Element {
  let (meta, count) = match &props.matches {
    Some((total, _)) => (matches_text(ctx, *total), Some(*total)),
    None => (summary_text(ctx, props.summary), None),
  };
  let content = row_content(
    (Icon::List, picked),
    (None, &ctx.t("runs.conversation.rail.allSteps"), count),
    &meta,
  );
  Frame {
    id: "conversation-rail-all".into(),
    picked,
    content,
  }
  .on(props.pick(StepFilter(None)))
}

/// A step's row: pressable once it ran.
fn step_row(ctx: &Ctx, props: &RailProps<'_>, step: &RailStep, (picked, matches): (bool, Option<usize>)) -> Element {
  let strong = picked
    || matches!(
      step.state,
      RailState::Working | RailState::Waits(_) | RailState::Failed { .. }
    );
  let meta = match matches {
    Some(0) => ctx.t("runs.conversation.rail.noMatch"),
    Some(count) => {
      let key = if count == 1 {
        "runs.conversation.rail.matches.one"
      } else {
        "runs.conversation.rail.matches.other"
      };
      ctx.t_args(
        key,
        [("agent", &*who(ctx, step)), ("count", count.to_string().as_str())],
      )
    }
    None => step_meta(ctx, step),
  };
  let number = format!("{:02}", step.number);
  let content = row_content(
    (glyph(&step.state), strong),
    (Some(&number), &step.name, matches.filter(|count| *count > 0)),
    &meta,
  );
  let id = format!("conversation-rail-step-{}", step.number);
  if !step.started() {
    return Column::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .padding_horizontal(ExtraSpacing::Inline.role())
      .opacity(NOT_STARTED_OPACITY)
      .describe("started", "false")
      .child(content.not_started())
      .into();
  }
  Frame { id, picked, content }.on(props.pick(StepFilter(Some(step.key.clone()))))
}

/// A rail row's parts: glyph, number, name, matches and meta.
struct Content {
  glyph: Icon,
  strong: bool,
  number: Option<String>,
  name: Arc<str>,
  count: Option<usize>,
  meta: Arc<str>,
}

fn row_content(
  (glyph, strong): (Icon, bool),
  (number, name, count): (Option<&str>, &str, Option<usize>),
  meta: &str,
) -> Content {
  Content {
    glyph,
    strong,
    number: number.map(str::to_owned),
    name: name.into(),
    count,
    meta: meta.into(),
  }
}

impl Content {
  /// The row's line: `$text` while it is strong, `$muted` otherwise; `name_color` for the name.
  fn render(self, name_color: PaletteColor) -> Element {
    let tone = if self.strong {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    let name = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .with_children(self.number.map(|number| {
        Text::new(&number)
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted)
          .nowrap()
      }))
      .child(
        Text::new(&self.name)
          .variant(TypographyStyle::FieldLabel)
          .color(name_color)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .min_width(Dimension::Px(0.0))
          .flex_shrink(1.0),
      )
      .with_children(self.count.map(|count| {
        Row::new().flex(1.0).child(Spacer::new().flex(1.0)).child(
          Text::new(&count.to_string())
            .id("conversation-rail-matches")
            .variant(TypographyStyle::Mono)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        )
      }));
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Start)
      .child(icon(self.glyph, IconSize::Small, tone))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Mark.role())
          .child(name)
          .child(
            Text::new(&self.meta)
              .id("conversation-rail-meta")
              .variant(ExtraTypography::ReadBack.role())
              .nowrap()
              .text_overflow(TextOverflow::Elipsis)
              .min_width(Dimension::Px(0.0)),
          ),
      )
      .into()
  }

  fn not_started(self) -> Element {
    self.render(PaletteColor::TextMuted)
  }
}

/// A pickable row: `$hover` while picked or under the pointer.
struct Frame {
  id: String,
  picked: bool,
  content: Content,
}

impl Frame {
  fn on(self, press: impl Fn() + Send + Sync + 'static) -> Element {
    let name_color = if self.content.strong {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    let row = pressable(surface().id(self.id), press)
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .padding_horizontal(ExtraSpacing::Inline.role())
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .describe("picked", self.picked.to_string())
      .child(self.content.render(name_color));
    if self.picked {
      row.background(PaletteColor::AccentMuted).into()
    } else {
      row.into()
    }
  }
}

/// A step's state icon: what it waits for, else how it stands.
pub(super) fn glyph(state: &RailState) -> Icon {
  match state {
    RailState::NotStarted => Icon::CircleDashed,
    RailState::Working => Icon::Loader,
    RailState::Waits(WaitReason::Message) => Icon::MessageCircle,
    RailState::Waits(WaitReason::Plan | WaitReason::Approval) => Icon::Hand,
    RailState::Passed { .. } => Icon::Check,
    RailState::Failed { .. } => Icon::CircleX,
    RailState::Returned { .. } => Icon::Undo2,
    RailState::Stopped => Icon::CircleMinus,
  }
}

/// The step's agent, or *Tool* for a tool step.
fn who(ctx: &Ctx, step: &RailStep) -> Arc<str> {
  match (&step.agent, step.tool_step) {
    (Some(agent), false) => agent.clone(),
    _ => ctx.t("runs.conversation.rail.tool"),
  }
}

/// "Explorer · 41k · 6 min", "Planner · waiting for approval", "Tool · exit 1 · 2 min", "Builder ·
/// not started".
fn step_meta(ctx: &Ctx, step: &RailStep) -> Arc<str> {
  let who = who(ctx, step);
  let counted = step.latest.as_ref().map_or(0, |head| head.usage.counted());
  let tokens = compact_tokens(counted);
  let took = took(ctx, step);
  let (key, extra): (&str, Vec<(&str, String)>) = match (&step.state, step.tool_step) {
    (RailState::NotStarted, _) => ("runs.conversation.rail.notStarted", Vec::new()),
    (RailState::Working, _) => match step
      .latest
      .as_ref()
      .and_then(|head| head.attempt)
      .filter(|attempt| *attempt > 1)
    {
      Some(attempt) => (
        "runs.conversation.rail.workingAttempt",
        vec![("attempt", attempt.to_string())],
      ),
      None if step.tool_step => ("runs.conversation.rail.workingTool", Vec::new()),
      None if counted == 0 => ("runs.conversation.rail.workingNoTokens", Vec::new()),
      None => ("runs.conversation.rail.working", vec![("tokens", tokens)]),
    },
    (RailState::Waits(WaitReason::Message), _) => ("runs.conversation.rail.waitingAnswer", Vec::new()),
    (RailState::Waits(_), _) => ("runs.conversation.rail.waitingApproval", Vec::new()),
    (RailState::Passed { approved: true }, _) => ("runs.conversation.rail.approved", vec![("tokens", tokens)]),
    (RailState::Passed { .. }, true) => ("runs.conversation.rail.toolPassed", vec![("took", took)]),
    (RailState::Passed { .. }, false) => ("runs.conversation.rail.done", vec![("tokens", tokens), ("took", took)]),
    (RailState::Failed { exit: Some(code) }, true) => (
      "runs.conversation.rail.toolExit",
      vec![("code", code.to_string()), ("took", took)],
    ),
    (RailState::Failed { .. }, _) => ("runs.conversation.rail.failed", vec![("took", took)]),
    (RailState::Returned { to }, _) => ("runs.conversation.rail.returned", vec![("step", to.to_string())]),
    (RailState::Stopped, _) => ("runs.conversation.rail.stopped", Vec::new()),
  };
  let mut args: Vec<(&str, &str)> = vec![("agent", &*who)];
  args.extend(extra.iter().map(|(name, value)| (*name, value.as_str())));
  ctx.t_args(key, args)
}

/// How long the step's latest execution took, once it ended.
fn took(ctx: &Ctx, step: &RailStep) -> String {
  let Some(head) = &step.latest else {
    return String::new();
  };
  let Some((started, ended)) = head.started.zip(head.ended) else {
    return String::new();
  };
  // A clock set back while the step ran reads as no time, never as a negative one.
  let millis = u64::try_from(ended.unix_millis() - started.unix_millis()).unwrap_or_default();
  message_text(ctx, &display::duration(std::time::Duration::from_millis(millis))).to_string()
}

/// "7 executions · 412k tokens", "2 of 7 steps ran · 119k tokens" or "Queued · 7 steps".
fn summary_text(ctx: &Ctx, summary: RailSummary) -> Arc<str> {
  match summary {
    RailSummary::Queued { steps } => ctx.t_args(
      "runs.conversation.rail.all.queued",
      [("count", steps.to_string().as_str())],
    ),
    RailSummary::Partial { ran, steps, tokens } => ctx.t_args(
      "runs.conversation.rail.all.partial",
      [
        ("ran", ran.to_string().as_str()),
        ("count", steps.to_string().as_str()),
        ("tokens", compact_tokens(tokens).as_str()),
      ],
    ),
    RailSummary::Ran { executions, tokens } => ctx.t_args(
      if executions == 1 {
        "runs.conversation.rail.all.ran.one"
      } else {
        "runs.conversation.rail.all.ran.other"
      },
      [
        ("count", executions.to_string().as_str()),
        ("tokens", compact_tokens(tokens).as_str()),
      ],
    ),
  }
}

/// "4 matches".
fn matches_text(ctx: &Ctx, count: usize) -> Arc<str> {
  let key = if count == 1 {
    "runs.conversation.rail.all.matches.one"
  } else {
    "runs.conversation.rail.all.matches.other"
  };
  ctx.t_args(key, [("count", count.to_string().as_str())])
}
