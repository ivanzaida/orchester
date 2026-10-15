//! Cards in the chat (`BRx5g`, `K4AOD`, `uDqzB`): a decision card sits in the asking agent's message
//! — on `$raised` with `$border` while it waits, radius 8, padding 14: its glyph, title and status
//! label, then its body. A question the composer answers (*Answer in the box below*); a plan with
//! *Approve plan & continue*; a tool request with the approval dialog's answers
//! ([`super::approval_card`]). After the decision a card loses the raised fill and reads *Answered*,
//! *Approved* or *Denied*; a plan automation decided reads *Approved by automation* or *Changes
//! requested by automation*. The step the run ended on closes with its result card: border only, its
//! title and time, what was reported, and the ways to the Changes and the Summary. What an agent wrote
//! reads as Markdown ([`MarkdownText`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  run::{Answerer, PlanDecision, PlanReview, StepOutcome},
  tokens::compact_tokens,
};

use super::{
  super::plan_review::{Decision as PlanDecisionRequest, decision_action as plan_decision},
  Timeline,
  chat::{Avatar, chat_row},
  divider::{changes_link, link},
};
use crate::{
  display,
  navigation::route::TaskPageTab,
  tasks::{
    conversation::{Author, Entry, EntryState},
    timeline::{NextStep, PlanCard, ResultCard},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    markdown::MarkdownText,
    message_text,
    pressable::{pressable, surface},
    status_label::compact_status_label,
  },
};

/// Lines of a plan shown before *Show the whole plan* (`K4AOD`).
const PLAN_LINES: usize = 6;

/// A card's frame: raised while it waits for the person, a border only once decided.
pub(super) fn frame(id: String, waiting: bool, header: Element, body: Vec<Element>) -> Column {
  let card = Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Group.role())
    .spacing(ExtraSpacing::Inline.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .describe("waiting", waiting.to_string())
    .child(header)
    .with_children(body);
  if waiting {
    card.background(PaletteColor::SurfaceRaised)
  } else {
    card
  }
}

/// Glyph, title (12/500) and the status label at the right (`PZNJa`). In a narrow card the title wraps
/// and the status label stays whole.
pub(super) fn header(glyph: Icon, title: &str, (state_glyph, state): (Icon, Arc<str>)) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextPrimary))
    .child(
      Text::new(title)
        .variant(ExtraTypography::NoticeHeading.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .child(compact_status_label(state_glyph, &state))
    .into()
}

/// *Builder asks you* (`YKutc` in `BRx5g`) in the agent's message: the question and, while it waits,
/// that the composer answers it; pressing that line puts the focus there.
pub(super) fn question(ctx: &mut Ctx, (entry, _step): (&Entry, Option<&Arc<str>>), timeline: &Timeline) -> Element {
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
  let hint = open.then(|| {
    let focus = timeline.focus.clone();
    Element::from(
      pressable(surface().id("conversation-reply"), move || focus.set(true))
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::ArrowDown, IconSize::Reason, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t_args("runs.conversation.card.answerBelow", [("agent", &*agent)]))
            .variant(ExtraTypography::Hint.role())
            .flex_shrink(1.0),
        ),
    )
  });
  let question = MarkdownText::message(format!("conversation-question-text-{}", entry.id), entry.body.clone());
  let body = std::iter::once(question.mount(ctx)).chain(hint).collect();
  let card = frame(
    format!("conversation-question-{}", entry.id),
    open,
    header(Icon::MessageCircleQuestion, &title, state),
    body,
  )
  .into();
  let meta = ctx.t_args(
    "runs.conversation.asksMeta",
    [("time", display::clock(entry.at).as_str())],
  );
  chat_row(
    format!("conversation-asks-{}", entry.id),
    Avatar::Question,
    (&agent, &meta),
    vec![card],
  )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlanCardProps {
  pub(super) card: PlanCard,
  pub(super) task_title: Arc<str>,
}

impl lurq::app::component::DevtoolsInspectable for PlanCardProps {}

/// *Plan v1 · <task>* (`M4cGG` in `K4AOD`) in the planner's message: the first lines of the plan,
/// *Show the whole plan*, and *Approve plan & continue* while it waits; *Request changes* is the
/// composer's.
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
    let state = match &plan.review {
      None if props.card.waiting => (Icon::Hand, ctx.t("runs.conversation.card.waitingApproval")),
      None => (Icon::CircleMinus, ctx.t("runs.conversation.card.superseded")),
      Some(review) => decided_state(ctx, review),
    };
    let lines: Vec<&str> = plan.text.lines().collect();
    let whole = self.whole.get() || lines.len() <= PLAN_LINES;
    let shown: Arc<str> = if whole {
      plan.text.as_str().into()
    } else {
      lines[..PLAN_LINES].join("\n").into()
    };
    // The plan's first lines, or all of it, as Markdown (`s0XDoI` drew them as lines of text).
    let mut body: Vec<Element> = vec![MarkdownText::agent_text("conversation-plan-lines", shown).mount(ctx)];
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
            .color(PaletteColor::TextPrimary)
            .nowrap(),
          )
          .into(),
      );
    }
    if props.card.waiting {
      body.push(approve(ctx, &props.card, &action));
    }
    let card = frame(
      format!("conversation-plan-{}", plan.version),
      props.card.waiting,
      header(Icon::ListChecks, &title, state),
      body,
    )
    .into();
    chat_row(
      format!("conversation-plan-row-{}", plan.version),
      Avatar::Plan,
      (&props.card.agent, &display::clock(plan.created_at)),
      vec![card],
    )
  }
}

/// A decided plan's state: who decided when automation did, as an answered request's card says it; a
/// person's decision reads plain *Approved* or *Changes requested*.
fn decided_state(ctx: &Ctx, review: &PlanReview) -> (Icon, Arc<str>) {
  let (glyph, key) = match (&review.decision, review.by) {
    (PlanDecision::Approved, Answerer::Person) => (Icon::Check, "runs.conversation.card.approved"),
    (PlanDecision::Approved, Answerer::Automation) => (Icon::Check, "runs.conversation.card.approvedByAutomation"),
    (PlanDecision::ChangesRequested { .. }, Answerer::Person) => {
      (Icon::Undo2, "runs.conversation.card.changesRequested")
    }
    (PlanDecision::ChangesRequested { .. }, Answerer::Automation) => {
      (Icon::Undo2, "runs.conversation.card.changesRequestedByAutomation")
    }
  };
  (glyph, ctx.t(key))
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
        .spacing(ExtraSpacing::Inline.role())
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
  // What the agent said needs the agent's name: a step whose profile is gone says nothing of it.
  let (Some(agent), false) = (&card.agent, card.tool_step) else {
    return meta;
  };
  let reported = match card.outcome {
    Some(StepOutcome::Done) => "runs.conversation.result.reportedPassed",
    Some(StepOutcome::Failed) => "runs.conversation.result.reportedFailed",
    Some(StepOutcome::Unverified) => "runs.conversation.result.reportedUnverified",
    Some(StepOutcome::Question) | None => return meta,
  };
  let reported = ctx.t_args(reported, [("agent", &**agent)]);
  if meta.is_empty() {
    return reported;
  }
  ctx.t_args(
    "runs.conversation.result.withReport",
    [("reported", &*reported), ("meta", &*meta)],
  )
}

/// The result card of the step the run ended on (`AlHBn` in `uDqzB`): *Docs & cleanup done*, *Review
/// passed* or *Run checks failed* with how long it took and its tokens, what was reported, and the
/// ways to the Changes and the Summary.
pub(super) fn result(ctx: &mut Ctx, card: &ResultCard, timeline: &Timeline) -> Element {
  let (glyph, title) = result_title(ctx, card);
  let meta = result_meta(ctx, card);
  let changes = changes_link(ctx, timeline, card.edited);
  let summary = link(
    ctx,
    timeline,
    ("conversation-result-summary", ctx.t("runs.conversation.result.summary")),
    TaskPageTab::Summary,
  );
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextPrimary))
    .child(
      Text::new(&title)
        .variant(ExtraTypography::NoticeHeading.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&meta)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  Column::new()
    .id(format!("conversation-result-{}", card.execution))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Group.role())
    .spacing(SpacingSize::Sm)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .child(head)
    .with_children(card.reported.as_ref().map(|reported| {
      MarkdownText::agent_text("conversation-reported", reported.clone())
        .color(PaletteColor::TextSecondary)
        .mount(ctx)
    }))
    .child(
      Row::new()
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(changes)
        .child(summary),
    )
    .into()
}
