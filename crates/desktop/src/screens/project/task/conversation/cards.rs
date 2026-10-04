//! Cards for decisions and results in the timeline (`ZxYGh`, `haIEI`, `OlLGD`, `AzKYQ`, `hep0l`): on
//! `$raised` with `$border`, radius 8 and padding 14, a header (glyph on `$hover`, title, step and
//! time, status label at the right), the body indented 36 and the actions under it. A question the
//! composer answers; a plan with *Approve plan & continue*; a tool request with the approval dialog's
//! answers ([`super::approval_card`]). After the decision a card loses the raised fill and reads
//! *Answered*, *Approved* or *Denied*. A step's result card has a border only; the run's end is a
//! centred hairline marker.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  run::{PlanDecision, StepOutcome},
  tokens::compact_tokens,
};

use super::{
  super::plan_review::{Decision as PlanDecisionRequest, decision_action as plan_decision},
  Timeline, nav,
};
use crate::{
  display,
  navigation::route::{Page, TaskPageTab},
  tasks::{
    conversation::{Author, Entry, EntryState},
    timeline::{NextStep, PlanCard, ResultCard, RunEnd},
  },
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Glyph tile of a card's header (`ZxYGh`); lines of a plan shown before *Show the whole plan*
/// (`haIEI`).
const GLYPH_SIZE: f32 = 24.0;
const PLAN_LINES: usize = 6;

/// A card's frame: raised while it waits for the person, a border only once decided.
pub(super) fn frame(id: String, waiting: bool, header: Element, body: Vec<Element>) -> Element {
  let card = Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Inline.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .describe("waiting", waiting.to_string())
    .child(header)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_left(ExtraSpacing::TimelineIndent.role())
        .spacing(ExtraSpacing::Inline.role())
        .with_children(body),
    );
  if waiting {
    card.background(PaletteColor::SurfaceRaised).into()
  } else {
    card.into()
  }
}

/// Glyph on `$hover`, title (12/500), meta, and the status label at the right.
pub(super) fn header(glyph: Icon, title: &str, meta: &str, (state_glyph, state): (Icon, Arc<str>)) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::AccentMuted)
        .rounded(RadiusSize::Sm)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(glyph, IconSize::Entry, PaletteColor::TextPrimary)),
    )
    .child(Text::new(title).variant(ExtraTypography::NoticeHeading.role()).nowrap())
    .child(
      Text::new(meta)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(compact_status_label(state_glyph, &state))
    .into()
}

/// *Builder asks you* (`ZxYGh`) with its step and time: the question, and *Reply*, which puts the
/// focus in the composer.
pub(super) fn question(ctx: &Ctx, (entry, step): (&Entry, &Arc<str>), timeline: &Timeline) -> Element {
  let open = entry.state == EntryState::Question { open: true };
  let agent = match &entry.author {
    Author::Agent(agent) | Author::YouTo(agent) => agent.clone(),
    Author::You => "".into(),
  };
  let state = if open {
    (Icon::MessageCircle, ctx.t("runs.conversation.card.waitingAnswer"))
  } else {
    (Icon::Check, ctx.t("runs.conversation.card.answered"))
  };
  let title = ctx.t_args("runs.conversation.card.asks", [("agent", &*agent)]);
  let time = display::clock(entry.at);
  let meta = if step.is_empty() {
    time.as_str().into()
  } else {
    ctx.t_args(
      "runs.conversation.agentMeta",
      [("step", &**step), ("time", time.as_str())],
    )
  };
  let reply = open.then(|| {
    let focus = timeline.focus.clone();
    Element::from(
      pressable(surface().id("conversation-reply"), move || focus.set(true)).child(
        Text::new(&ctx.t("conversation.reply"))
          .variant(TypographyStyle::Link)
          .nowrap(),
      ),
    )
  });
  let body = std::iter::once(
    Text::new(&entry.body)
      .variant(ExtraTypography::TimelineMessage.role())
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .into(),
  )
  .chain(reply)
  .collect();
  frame(
    format!("conversation-question-{}", entry.id),
    open,
    header(Icon::MessageCircle, &title, &meta, state),
    body,
  )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlanCardProps {
  pub(super) card: PlanCard,
  pub(super) task_title: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for PlanCardProps {}

/// *Plan v1 · <task>* (`haIEI`): the first lines of the plan, *Show the whole plan*, and *Approve
/// plan & continue* while it waits; *Request changes* is the composer's.
pub(super) struct PlanCardView {
  whole: Signal<bool>,
}

impl Component for PlanCardView {
  type Props = PlanCardProps;

  fn create(ctx: &mut Ctx) -> Self {
    plan_decision(ctx);
    Self {
      whole: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PlanCardProps>().clone();
    let action = plan_decision(ctx);
    let plan = &props.card.plan;
    let version = plan.version.to_string();
    let title = ctx.t_args(
      "runs.plan.title",
      [("version", version.as_str()), ("task", &*props.task_title)],
    );
    let meta = ctx.t_args(
      "runs.conversation.agentMeta",
      [
        ("step", &*props.card.step),
        ("time", display::clock(plan.created_at).as_str()),
      ],
    );
    let state = match plan.decision() {
      None if props.card.waiting => (Icon::Hand, ctx.t("runs.conversation.card.waitingApproval")),
      None => (Icon::CircleMinus, ctx.t("runs.conversation.card.superseded")),
      Some(PlanDecision::Approved) => (Icon::Check, ctx.t("runs.conversation.card.approved")),
      Some(PlanDecision::ChangesRequested { .. }) => (Icon::Undo2, ctx.t("runs.conversation.card.changesRequested")),
    };
    let lines: Vec<&str> = plan.text.lines().collect();
    let whole = self.whole.get() || lines.len() <= PLAN_LINES;
    let shown = if whole { &lines[..] } else { &lines[..PLAN_LINES] };
    // One text a line, 6 apart (`KMmct` in `haIEI`).
    let mut body: Vec<Element> = vec![
      Column::new()
        .id("conversation-plan-lines")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .with_children(shown.iter().map(|line| {
          Text::new(line)
            .variant(ExtraTypography::TimelineMessage.role())
            .width(Dimension::Pct(100.0))
            .selectable(true)
        }))
        .into(),
    ];
    if !whole {
      let open = self.whole.clone();
      body.push(
        pressable(surface().id("conversation-plan-whole"), move || open.set(true))
          .child(
            Text::new(&ctx.t_args(
              "runs.conversation.card.wholePlan",
              [("count", lines.len().to_string().as_str())],
            ))
            .variant(TypographyStyle::Link)
            .nowrap(),
          )
          .into(),
      );
    }
    if props.card.waiting {
      body.push(approve(ctx, &props.card, &action));
    }
    frame(
      format!("conversation-plan-{}", plan.version),
      props.card.waiting,
      header(Icon::ListChecks, &title, &meta, state),
      body,
    )
  }
}

/// *Approve plan & continue* with what approving does beside it (`qjMHX`), and why it was not
/// carried out.
fn approve(ctx: &Ctx, card: &PlanCard, action: &super::super::plan_review::DecisionAction) -> Element {
  let state = action.state().get();
  let pending = state.is_pending();
  let label = if pending {
    ctx.t("runs.plan.approving")
  } else {
    ctx.t("runs.plan.approve")
  };
  let mut button = ActionButton::new(label)
    .icon(Icon::Check)
    .primary()
    .id("conversation-approve-plan");
  button = if pending || state.data.is_some() {
    button.disabled()
  } else {
    let (action, request) = (
      action.clone(),
      PlanDecisionRequest {
        run: card.plan.run,
        execution: card.plan.execution,
        version: card.plan.version,
        decision: PlanDecision::Approved,
      },
    );
    button.on_click(move || action.run(request.clone()))
  };
  let meaning = Text::new(&approve_meaning(ctx, card))
    .id("conversation-approve-meaning")
    .variant(TypographyStyle::Caption)
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .child(Row::new().flex_shrink(0.0).child(button.render(ctx)))
        .child(meaning),
    )
    .with_children(
      state
        .error
        .filter(|_| !pending)
        .map(|error| field_error(&message_text(ctx, &error))),
    )
    .into()
}

/// What approving `card` starts: the step after it and its agent.
fn approve_meaning(ctx: &Ctx, card: &PlanCard) -> Arc<str> {
  let version = card.plan.version.to_string();
  match &card.next {
    Some(NextStep {
      step,
      agent: Some(agent),
    }) => ctx.t_args(
      "runs.conversation.card.approveStarts",
      [("version", version.as_str()), ("agent", &**agent), ("step", &**step)],
    ),
    Some(NextStep { step, agent: None }) => ctx.t_args(
      "runs.conversation.card.approveStartsTool",
      [("version", version.as_str()), ("step", &**step)],
    ),
    None => ctx.t_args("runs.conversation.card.approveLast", [("version", version.as_str())]),
  }
}

/// *Review passed* for a step that judges the work, *Research done* for another, *Run checks
/// failed*; and its glyph.
fn result_title(ctx: &Ctx, card: &ResultCard) -> (Icon, Arc<str>) {
  let key = match (card.passed, card.verdict) {
    (true, true) => "runs.conversation.result.passed",
    (true, false) => "runs.conversation.result.done",
    (false, _) => "runs.conversation.result.failed",
  };
  let glyph = if card.passed { Icon::CircleCheck } else { Icon::CircleX };
  (glyph, ctx.t_args(key, [("step", &*card.step)]))
}

/// *Reviewer reported passed · 4 min · 96k tokens*: what the agent's result said, how long the step
/// took and its tokens; a tool step's time alone.
fn result_meta(ctx: &Ctx, card: &ResultCard) -> Arc<str> {
  let took = card.took_millis.map(|millis| {
    // A clock set back while the step ran reads as no time, never as a negative one.
    let millis = u64::try_from(millis).unwrap_or_default();
    message_text(ctx, &display::duration(std::time::Duration::from_millis(millis)))
  });
  let meta = match (took, card.tool_step) {
    (Some(took), false) => ctx.t_args(
      "runs.conversation.result.meta",
      [("took", &*took), ("tokens", compact_tokens(card.tokens).as_str())],
    ),
    (Some(took), true) => took,
    (None, _) => "".into(),
  };
  let reported = match card.outcome.filter(|_| !card.tool_step) {
    Some(StepOutcome::Done) => "runs.conversation.result.reportedPassed",
    Some(StepOutcome::Failed) => "runs.conversation.result.reportedFailed",
    Some(StepOutcome::Question) | None => return meta,
  };
  let reported = ctx.t_args(reported, [("agent", &*card.agent)]);
  if meta.is_empty() {
    return reported;
  }
  ctx.t_args(
    "runs.conversation.result.withReport",
    [("reported", &*reported), ("meta", &*meta)],
  )
}

/// A step's result card (`AzKYQ`, `hep0l`, `LhLL8`): *Review passed*, *Research done* or *Run checks
/// failed*, what the agent's result said, how long it took and its tokens, what was reported, and
/// the ways to the Changes and the Summary.
pub(super) fn result(ctx: &mut Ctx, card: &ResultCard, timeline: &Timeline) -> Element {
  let nav = nav(ctx);
  let (glyph, title) = result_title(ctx, card);
  let meta = result_meta(ctx, card);
  let link = |id: &'static str, label: Arc<str>, page: Page| {
    nav
      .link(page)
      .on(surface().id(id))
      .child(Text::new(&label).variant(TypographyStyle::Link).nowrap())
  };
  let links = Row::new()
    .spacing(ExtraSpacing::Stack.role())
    .child(link(
      "conversation-result-changes",
      ctx.t("runs.conversation.result.changes"),
      Page::Task(timeline.task, TaskPageTab::Changes),
    ))
    .child(link(
      "conversation-result-summary",
      ctx.t("runs.conversation.result.summary"),
      Page::Task(timeline.task, TaskPageTab::Summary),
    ));
  // The title's meta follows it; what was reported and the links line up under the title (`AzKYQ`).
  let body = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&title)
            .variant(ExtraTypography::NoticeHeading.role())
            .nowrap(),
        )
        .child(
          Text::new(&meta)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextMuted)
            .nowrap(),
        ),
    )
    .with_children(card.reported.as_deref().map(|reported| {
      Text::new(reported)
        .variant(ExtraTypography::Explanation.role())
        .width(Dimension::Pct(100.0))
        .selectable(true)
    }))
    .child(links);
  Row::new()
    .id(format!("conversation-result-{}", card.execution))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
    .child(body)
    .into()
}

/// The run's end (`AzKYQ`): a centred line between hairlines.
pub(super) fn end_marker(ctx: &Ctx, end: &RunEnd, (run, at): (u64, orchester_core::time::Timestamp)) -> Element {
  let (number, time) = (run.to_string(), display::clock(at));
  let text = match end {
    RunEnd::Ready => ctx.t_args(
      "runs.conversation.end.ready",
      [("run", number.as_str()), ("time", time.as_str())],
    ),
    RunEnd::Merged(branch) => ctx.t_args(
      "runs.conversation.end.merged",
      [("run", number.as_str()), ("branch", &**branch), ("time", time.as_str())],
    ),
    RunEnd::Failed(step) => ctx.t_args(
      "runs.conversation.end.failed",
      [("run", number.as_str()), ("step", &**step), ("time", time.as_str())],
    ),
    RunEnd::Cancelled => ctx.t_args(
      "runs.conversation.end.cancelled",
      [("run", number.as_str()), ("time", time.as_str())],
    ),
  };
  let hairline = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(1.0))
      .background(ExtraColor::Hairline.role())
  };
  Row::new()
    .id("conversation-end")
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(hairline())
    .child(Text::new(&text).variant(TypographyStyle::Caption).nowrap())
    .child(hairline())
    .into()
}
