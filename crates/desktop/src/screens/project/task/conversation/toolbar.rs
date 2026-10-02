//! The timeline's toolbar (52 high, `RrScB`): the step filter (`ZGwlv`), *Show* (All, Messages,
//! Tools), *Show raw lines* in the Tools view (`q4fCXK`), the search field, whether the timeline
//! follows live (or the run waits for the person's decision), and *Details*, pressed while its pane
//! is open.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::id::StepExecutionId;

use super::{super::activity::DETAILS_WIDTH, ShowChoice, StepFilter, details::DetailsPane, segments::segments};
use crate::{
  tasks::{detail::RunView, timeline::view::Show},
  theme::{
    CONVERSATION_TOOLBAR_DECISION_STATE, CONVERSATION_TOOLBAR_ONE_LINE_MIN_WINDOW, CONVERSATION_TOOLBAR_RAW_SWITCH,
    ExtraColor, ExtraSpacing,
  },
  ui::{
    field::{select_style, styled_input},
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    status_label::compact_status_label,
    switch::Switch,
  },
};

/// Toolbar height, the step filter, the search field and *Details* (`RrScB`, `gNa91`).
const TOOLBAR_HEIGHT: f32 = 52.0;
const FILTER_WIDTH: f32 = 240.0;
const CONTROL_HEIGHT: f32 = 32.0;
const SEARCH_WIDTH: f32 = 180.0;

/// What the toolbar shows and changes.
pub(super) struct ToolbarState {
  pub(super) step: Signal<StepFilter>,
  pub(super) show: Signal<ShowChoice>,
  pub(super) query: Signal<String>,
  pub(super) raw: Signal<bool>,
  pub(super) details: DetailsPane,
  pub(super) details_open: bool,
  /// An agent is at work on the run.
  pub(super) live: bool,
  /// The run waits for the person's decision after its retries: it has not ended.
  pub(super) deciding: bool,
  pub(super) following: bool,
}

/// The toolbar across the tab's main column, with its side padding. While the window is too narrow
/// for one line, the step filter keeps the first with the live state and *Details*, and *Show*, *Show
/// raw lines* and the search go to a second.
pub(super) fn toolbar(ctx: &Ctx, run: &RunView, state: &ToolbarState) -> Element {
  let show_options = vec![
    (ShowChoice(Show::All), ctx.t("runs.conversation.show.all")),
    (ShowChoice(Show::Messages), ctx.t("runs.conversation.show.messages")),
    (ShowChoice(Show::Tools), ctx.t("runs.conversation.show.tools")),
  ];
  let tools = state.show.get().0 == Show::Tools;
  let raw = tools.then(|| {
    let (raw, on) = (state.raw.clone(), state.raw.get());
    Switch::new("activity-raw", ctx.t("runs.activity.showRaw"), on).render(move || raw.set(!on))
  });
  let follow = match (state.live, state.following) {
    (true, true) => compact_status_label(Icon::Radio, &ctx.t("runs.activity.following")),
    (true, false) => compact_status_label(Icon::CirclePause, &ctx.t("runs.activity.notFollowing")),
    (false, _) if state.deciding => compact_status_label(Icon::Hand, &ctx.t("runs.activity.waitingDecision")),
    (false, _) => compact_status_label(Icon::Check, &ctx.t("runs.activity.ended")),
  }
  .id("conversation-live-state");
  let show = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("runs.conversation.show.label")).variant(TypographyStyle::Caption))
    .child(segments("conversation-show", &state.show, show_options));
  let details = details_button(ctx, &state.details, state.details_open);
  let line = || {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
  };
  if one_line_fits(ctx, state, tools) {
    return line()
      .id("conversation-toolbar")
      .height(Dimension::Px(TOOLBAR_HEIGHT))
      .padding_horizontal(SpacingSize::Section)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(step_filter(ctx, run, &state.step, Dimension::Px(FILTER_WIDTH)))
      .child(show)
      .with_children(raw)
      .child(search(ctx, &state.query))
      .child(Spacer::new().flex(1.0))
      .child(follow)
      .child(details)
      .into();
  }
  Column::new()
    .id("conversation-toolbar")
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Section)
    .padding_vertical(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Stack.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      line()
        .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(step_filter(
          ctx,
          run,
          &state.step,
          Dimension::Pct(100.0),
        )))
        .child(Row::new().flex_shrink(0.0).child(follow))
        .child(Row::new().flex_shrink(0.0).child(details)),
    )
    .child(line().child(show).with_children(raw).child(search(ctx, &state.query)))
    .into()
}

/// Whether the window holds the toolbar on one line, beside the Details pane when it is open.
/// Reading the window's width re-renders the toolbar as the window resizes.
fn one_line_fits(ctx: &Ctx, state: &ToolbarState, tools: bool) -> bool {
  let mut needed = CONVERSATION_TOOLBAR_ONE_LINE_MIN_WINDOW;
  if state.details_open {
    needed += DETAILS_WIDTH;
  }
  if tools {
    needed += CONVERSATION_TOOLBAR_RAW_SWITCH;
  }
  if state.deciding {
    needed += CONVERSATION_TOOLBAR_DECISION_STATE;
  }
  ctx.window().logical_width() >= needed
}

/// *All steps · 3 executions* and one option per step execution that ran (`ZGwlv`), `width` wide.
fn step_filter(ctx: &Ctx, run: &RunView, filter: &Signal<StepFilter>, width: Dimension) -> Element {
  let executions: Vec<StepExecutionId> = run
    .run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .map(|execution| execution.id())
    .filter(|execution| run.invocations_of(*execution).next().is_some())
    .collect();
  let key = if executions.len() == 1 {
    "runs.activity.allSteps.one"
  } else {
    "runs.activity.allSteps.other"
  };
  let all = ctx.t_args(key, [("count", executions.len().to_string().as_str())]);
  let options = std::iter::once((StepFilter(None), all)).chain(
    executions
      .iter()
      .filter_map(|execution| Some((StepFilter(Some(*execution)), step_option(ctx, run, *execution)?))),
  );
  Element::from(
    Select::new(filter.clone())
      .options(options)
      .width(width)
      .height(Dimension::Px(CONTROL_HEIGHT))
      .style(select_style(ctx, CONTROL_HEIGHT)),
  )
  .id("activity-step-filter")
}

/// "03 · Implement · Builder", with its attempt when the run had more than one.
fn step_option(ctx: &Ctx, run: &RunView, execution: StepExecutionId) -> Option<Arc<str>> {
  let step = run.step_of(execution)?;
  let number = format!("{:02}", step.number);
  let args = [
    ("number", number.as_str()),
    ("step", &*step.name),
    ("agent", step.agent.as_deref().unwrap_or_default()),
  ];
  // A step a failure route re-executed has an execution per attempt, which the attempt tells apart.
  Some(
    match run
      .run
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(execution).is_some())
    {
      Some(attempt) if run.run.attempts().len() > 1 => {
        let attempt = attempt.number().to_string();
        let [number, step, agent] = args;
        ctx.t_args(
          "runs.activity.stepOptionAttempt",
          [number, step, agent, ("attempt", attempt.as_str())],
        )
      }
      _ => ctx.t_args("runs.activity.stepOption", args),
    },
  )
}

/// *Search this run…*; with a query, the field shows it and × clears it (`a5hF4`).
fn search(ctx: &Ctx, query: &Signal<String>) -> Element {
  let input = ctx.theme().form().input.clone();
  let field = styled_input(ctx, query.clone(), TypographyStyle::FieldLabel)
    .id("conversation-search")
    .single_line()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .placeholder(&ctx.t("runs.conversation.search"));
  let end: Element = if query.get().is_empty() {
    icon(Icon::Search, IconSize::Small, PaletteColor::TextSecondary).into()
  } else {
    let query = query.clone();
    pressable(surface().id("conversation-search-clear"), move || {
      query.set(String::new())
    })
    .child(icon(Icon::X, IconSize::Small, PaletteColor::TextSecondary))
    .into()
  };
  Row::new()
    .width(Dimension::Px(SEARCH_WIDTH))
    .height(Dimension::Px(CONTROL_HEIGHT))
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(input.background)
    .border(Border::inside(BorderSize::Sm, input.border.clone()))
    .rounded(input.radius)
    .child(field)
    .child(end)
    .into()
}

/// *Details* (panel-right, 32 high); pressed while its pane is open: `$hover` with a `$subtle`
/// stroke and panel-right-close (`gNa91`).
fn details_button(ctx: &Ctx, details: &DetailsPane, open: bool) -> Element {
  let (glyph, background, stroke) = if open {
    (
      Icon::PanelRightClose,
      PaletteColor::AccentMuted,
      PaletteColor::BorderStrong,
    )
  } else {
    (Icon::PanelRight, PaletteColor::SurfaceRaised, PaletteColor::Border)
  };
  pressable(surface().id("conversation-details"), details.toggle(open))
    .height(Dimension::Px(CONTROL_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(background)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .describe("open", open.to_string())
    .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(&ctx.t("runs.conversation.details"))
        .variant(TypographyStyle::Button)
        .nowrap(),
    )
    .into()
}
