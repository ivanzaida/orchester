//! A tool request inline in the timeline (`sACJF`, `pSVDW`): who asks for what and, while it waits, the
//! approval dialog's answers in one row (`KpZxA`): *Always allow…* at the left, then *Deny*, *Approve
//! similar in this run* and *Approve once*. Each answer goes the dialog's own way
//! ([`ApprovalDecision::of`], the same action and runner command); *Always allow…* opens the dialog's
//! Always allow step on this request. Over the answers, what it asks as the dialog shows it: the
//! category chip and why it asks (or the MCP server and tool), the command or the arguments whole in
//! their box, and where it runs. What the request offers is the dialog's too: an answer it does not
//! offer stays at 0.4, and why is said on one line under the row in the dialog's words. A request whose
//! agent can no longer receive an answer reads *Ended without an answer* and has no answers, as soon as
//! the runner says so. A card never shrinks while it is shown: once answered, it keeps the room its
//! answers took, so the chat around it does not move (ORC-71).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  run::{ApprovalAnswer, DecidedBy, PendingToolApproval, SimilarUnavailable, ToolApprovalDecision},
};

use super::{
  super::{
    approval_arguments::ArgumentsCache,
    approval_value::ValueRows,
    review_note::{review_line, reviewing_line},
    tool_approval::{
      Decision as ApprovalDecision, DecisionAction, Unanswered, always_blocked, chip,
      decision_action as approval_decision, quiet_reason, similar_unavailable,
    },
  },
  cards::{frame, header},
  chat::{Avatar, chat_row},
};
use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, pages::Shown},
  tasks::{
    approvals::{AlwaysBlocked, ShownApproval},
    timeline::ApprovalCard,
  },
  theme::{
    CONVERSATION_ANSWERS_ONE_LINE_MIN_CHAT, CONVERSATION_ANSWERS_ROW_MIN_CHAT, DISABLED_OPACITY, ExtraSpacing,
    ExtraTypography,
  },
  ui::{
    button::ActionButton,
    category_chip::category_chip,
    command_row::Copied,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
    value_box::CopyState,
  },
};

/// The answers' height (`KpZxA`).
const ANSWER_HEIGHT: f32 = 30.0;
/// How far a card's width or height may move before it counts as another.
const SIZE_EPSILON: f32 = 0.5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApprovalCardProps {
  pub(super) card: ApprovalCard,
  pub(super) answers: AnswersLayout,
}

/// How a card lays out its answers in the chat's width; no answer ever shrinks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnswersLayout {
  /// *Always allow…* at the left, the buttons at the right (`KpZxA`).
  OneLine,
  /// *Always allow…* on a line of its own, the buttons on one line under it at the right.
  AlwaysAbove,
  /// *Always allow…*, then each button on a row of its own, full width.
  Stacked,
}

impl AnswersLayout {
  /// The layout for a chat column `chat` wide.
  pub(super) fn for_chat(chat: f32) -> Self {
    if chat >= CONVERSATION_ANSWERS_ONE_LINE_MIN_CHAT {
      Self::OneLine
    } else if chat >= CONVERSATION_ANSWERS_ROW_MIN_CHAT {
      Self::AlwaysAbove
    } else {
      Self::Stacked
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for AnswersLayout {}

impl lurq::app::component::DevtoolsInspectable for ApprovalCardProps {}

/// The answers the card gives itself; *Always allow…* opens the dialog instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CardAnswer {
  Once,
  Similar,
  Deny,
}

impl lurq::app::component::DevtoolsInspectable for CardAnswer {}

impl CardAnswer {
  const fn answer(self) -> ApprovalAnswer {
    match self {
      Self::Once => ApprovalAnswer::Once,
      Self::Similar => ApprovalAnswer::Similar,
      Self::Deny => ApprovalAnswer::Deny,
    }
  }
}

/// The tallest a shown card was at the width it is laid out at; no height before it was laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Held {
  width: f32,
  height: f32,
}

pub(super) struct ApprovalCardView {
  requested: Signal<Option<CardAnswer>>,
  copy: CopyState,
  arguments: ArgumentsCache,
  frame: ElementRef,
  held: Signal<Held>,
}

impl Component for ApprovalCardView {
  type Props = ApprovalCardProps;

  fn create(ctx: &mut Ctx) -> Self {
    approval_decision(ctx);
    Self {
      requested: ctx.signal(None),
      copy: CopyState {
        copied: ctx.signal(Copied::default()),
        hovered: ctx.signal(None),
      },
      arguments: ArgumentsCache::default(),
      frame: ElementRef::new(),
      held: ctx.signal(Held::default()),
    }
  }

  /// Keeps the tallest height the card was laid out at. At another width (a resized window, Details
  /// opened) the answers wrap otherwise, so the card measures its own height again first.
  fn after_layout(&self) {
    if !self.frame.is_attached() {
      return;
    }
    let (width, height) = (self.frame.width(), self.frame.height());
    let held = self.held.get_untracked();
    let next = if (width - held.width).abs() > SIZE_EPSILON {
      let height = if held.height > 0.0 { 0.0 } else { height };
      Held { width, height }
    } else if height > held.height + SIZE_EPSILON {
      Held { width, height }
    } else {
      return;
    };
    self.held.set(next);
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ApprovalCardProps>().clone();
    let action = approval_decision(ctx);
    let request = &props.card.request;
    // The runner said it no longer waits, or was answered already, before the timeline is read again.
    let refused = action.state().get().error;
    let ended = refused.as_ref().is_some_and(|unanswered| unanswered.ended(&request.id));
    let answered = refused
      .as_ref()
      .is_some_and(|unanswered| unanswered.answered(&request.id));
    let waiting = props.card.decided.is_none() && !ended && !answered;
    let title = ctx.t_args(
      if request.command {
        "runs.conversation.card.asksCommand"
      } else {
        "runs.conversation.card.asksMcp"
      },
      [("agent", request.agent.as_str())],
    );
    let meta = ctx.t_args(
      "runs.conversation.agentMeta",
      [
        ("step", &*props.card.step),
        ("time", display::clock(request.requested_at).as_str()),
      ],
    );
    let (shown, reviewing) = shown_request(ctx, &request.id);
    let state = match props.card.decided {
      None if ended => (Icon::CirclePause, ctx.t("runs.conversation.card.ended")),
      None if answered => (Icon::Check, ctx.t("runs.conversation.card.answered")),
      None if reviewing => (Icon::Bot, ctx.t("runs.conversation.card.reviewing")),
      None => (Icon::Hand, ctx.t("runs.conversation.card.waitingAnswer")),
      Some(Some(decision)) => answered_state(ctx, decision, props.card.decided_by),
      Some(None) => (Icon::CirclePause, ctx.t("runs.conversation.card.ended")),
    };
    let mut body = self.request_body(ctx, request, shown.as_ref());
    let review = props
      .card
      .review
      .as_ref()
      .or_else(|| shown.as_ref().and_then(|shown| shown.review.as_ref()));
    body.extend(review.map(|review| review_line(ctx, format!("conversation-approval-review-{}", request.id), review)));
    body.extend(
      (reviewing && waiting).then(|| reviewing_line(ctx, format!("conversation-approval-reviewing-{}", request.id))),
    );
    if waiting {
      let queue = ctx.use_context::<ApprovalQueue>();
      body.push(self.answers(ctx, (&props.card, shown.as_ref(), props.answers), &action, queue));
    }
    let card = frame(
      format!("conversation-approval-{}", request.id),
      waiting,
      header(Icon::Hand, &title, state),
      body,
    )
    .ref_element(self.frame.clone())
    .min_height(Dimension::Px(self.held.get().height * 0.0))
    .into();
    chat_row(
      format!("conversation-request-{}", request.id),
      Avatar::Request,
      (request.agent.as_str(), &meta),
      vec![card],
    )
  }
}

impl ApprovalCardView {
  /// What the request asks (`sACJF`): its chip and why it asks, then the command whole in its box, or
  /// an MCP call's arguments; then where it runs, as the dialog names it.
  fn request_body(&self, ctx: &Ctx, request: &PendingToolApproval, shown: Option<&ShownApproval>) -> Vec<Element> {
    let values = ValueRows::new(ctx, &self.copy, &self.arguments);
    let heading = heading(ctx, request, shown);
    let mut body = Vec::new();
    match (request.command, request.scope.as_deref()) {
      (true, _) => {
        let id = format!("conversation-approval-command-{}", request.id);
        body.push(values.under((heading, &id), &request.action));
      }
      (false, Some(arguments)) => {
        let id = format!("conversation-approval-arguments-{}", request.id);
        body.push(values.arguments_under((heading, &id), &request.id, arguments));
      }
      (false, None) => body.push(heading),
    }
    let place = if request.command {
      shown
        .and_then(|shown| shown.cwd.as_deref())
        .or(request.target.as_deref())
        .map(|folder| ctx.t_args("runs.conversation.card.workingDirectory", [("folder", folder)]))
    } else {
      request
        .target
        .as_deref()
        .map(|target| ctx.t_args("runs.conversation.card.target", [("target", target)]))
    };
    body.extend(place.map(|place| {
      Text::new(&place)
        .id("conversation-approval-place")
        .variant(TypographyStyle::Caption)
        .into()
    }));
    body
  }
}

/// An answered request's state: who answered when automation or auto mode did, in the words of the
/// run's Approvals; a person's answer reads plain *Approved* or *Denied*. What Orchester answered
/// itself, the profile among them, has no card (`tasks::timeline`).
fn answered_state(ctx: &Ctx, decision: ToolApprovalDecision, by: Option<DecidedBy>) -> (Icon, Arc<str>) {
  let key = match (decision, by) {
    (ToolApprovalDecision::Approved, Some(DecidedBy::Automation)) => "runs.summary.approvals.byAutomation",
    (ToolApprovalDecision::Approved, Some(DecidedBy::Reviewer)) => "runs.summary.approvals.byReviewer",
    (ToolApprovalDecision::Approved, _) => "runs.conversation.card.approved",
    (ToolApprovalDecision::Denied, Some(DecidedBy::AutomationDeny)) => "runs.summary.approvals.deniedByAutomation",
    (ToolApprovalDecision::Denied, _) => "runs.conversation.card.denied",
  };
  let glyph = match decision {
    ToolApprovalDecision::Approved => Icon::Check,
    ToolApprovalDecision::Denied => Icon::X,
  };
  (glyph, ctx.t(key))
}

/// The line over the request's box: a command's chip and why it asks (the dialog's), or the MCP
/// server's chip and the tool.
fn heading(ctx: &Ctx, request: &PendingToolApproval, shown: Option<&ShownApproval>) -> Element {
  let row = Row::new()
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center);
  let row = match (request.command, shown) {
    (true, Some(shown)) => row
      .child(chip(ctx, "conversation-approval-category", shown))
      .with_children(shown.choices.reason.as_ref().map(|reason| quiet_reason(ctx, reason))),
    (true, None) => row.child(Text::new(&ctx.t("runs.toolApproval.command")).variant(TypographyStyle::Caption)),
    (false, _) => row
      .child(category_chip(
        "conversation-approval-category",
        Icon::Plug,
        &request.server,
      ))
      .child(
        Text::new(&request.action)
          .variant(ExtraTypography::MonoValue.role())
          .nowrap(),
      ),
  };
  row.into()
}

/// The request with its choices, as the project's waiting requests know it, and whether auto mode's
/// reviewer is still judging it (the person may decide all the same).
fn shown_request(ctx: &mut Ctx, id: &str) -> (Option<ShownApproval>, bool) {
  let Some(tasks) = ctx
    .use_context::<Shown>()
    .and_then(|shown| shown.tasks.ready().cloned())
  else {
    return (None, false);
  };
  let find = |list: &[ShownApproval]| list.iter().find(|pending| pending.approval.id == id).cloned();
  match find(&tasks.reviewing_tool_approvals) {
    Some(reviewing) => (Some(reviewing), true),
    None => (find(&tasks.pending_tool_approvals), false),
  }
}

impl CardAnswer {
  /// Its button's id, icon, label, and label while it is under way.
  const fn look(self) -> (&'static str, Icon, &'static str, &'static str) {
    match self {
      Self::Deny => (
        "conversation-approval-deny",
        Icon::X,
        "runs.toolApproval.deny",
        "runs.toolApproval.denying",
      ),
      Self::Similar => (
        "conversation-approval-similar",
        Icon::CopyCheck,
        "runs.toolApproval.wider.similar",
        "runs.toolApproval.approving",
      ),
      Self::Once => (
        "conversation-approval-approve",
        Icon::Check,
        "runs.toolApproval.approveOnce",
        "runs.toolApproval.approving",
      ),
    }
  }
}

/// What the request offers beyond Approve once and Deny: `Err` with why not; `None` while its choices
/// cannot be read.
struct Offers {
  similar: Option<Result<(), SimilarUnavailable>>,
  always: Option<Result<(), AlwaysBlocked>>,
}

impl Offers {
  fn of(shown: Option<&ShownApproval>) -> Self {
    Self {
      similar: shown.map(|shown| {
        shown
          .choices
          .similar
          .as_ref()
          .map(|_| ())
          .map_err(|unavailable| *unavailable)
      }),
      always: shown.map(|shown| shown.choices.always.as_ref().map(|_| ()).map_err(|blocked| *blocked)),
    }
  }

  /// Why an answer is unavailable, in the dialog's words, on one line: Approve similar's reason, else
  /// Always allow's.
  fn reason(&self, ctx: &Ctx) -> Option<Arc<str>> {
    self
      .similar
      .and_then(Result::err)
      .map(|unavailable| ctx.t(similar_unavailable(unavailable)))
      .or_else(|| {
        self
          .always
          .and_then(Result::err)
          .map(|blocked| always_blocked(ctx, blocked))
      })
  }
}

impl ApprovalCardView {
  /// The answers as `layout` places them, why one is unavailable, and why an answer was not carried out.
  fn answers(
    &self,
    ctx: &Ctx,
    (card, shown, layout): (&ApprovalCard, Option<&ShownApproval>, AnswersLayout),
    action: &DecisionAction,
    queue: Option<ApprovalQueue>,
  ) -> Element {
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    let offers = Offers::of(shown);
    let task = shown.and_then(|shown| shown.task_id);
    let stacked = layout == AnswersLayout::Stacked;
    let button =
      |answer: CardAnswer, offered: bool| self.answer(ctx, (card, action, task), (answer, busy, offered), stacked);
    let always = queue.filter(|_| matches!(offers.always, Some(Ok(()))) && !busy);
    let always = always_action(ctx, &card.request.id, card.request.command, always);
    let buttons = [
      button(CardAnswer::Deny, true),
      button(CardAnswer::Similar, matches!(offers.similar, Some(Ok(())))),
      button(CardAnswer::Once, true),
    ];
    let line = |first: Option<Element>| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(first)
        .child(Spacer::new().flex(1.0))
        .with_children(buttons.clone())
        .into()
    };
    let lines: Vec<Element> = match layout {
      AnswersLayout::OneLine => vec![line(Some(always))],
      AnswersLayout::AlwaysAbove => vec![always, line(None)],
      AnswersLayout::Stacked => std::iter::once(always).chain(buttons.clone()).collect(),
    };
    let reason = offers.reason(ctx).map(|reason| {
      let text = Text::new(&reason)
        .id("conversation-approval-unavailable")
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0));
      // On one line beside the answers; a narrow card wraps it rather than cut it.
      if layout == AnswersLayout::OneLine {
        text.nowrap().text_overflow(TextOverflow::Elipsis)
      } else {
        text
      }
    });
    Column::new()
      .id("conversation-approval-answers")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(lines)
      .with_children(reason)
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !busy)
          .and_then(Unanswered::failure)
          .map(|error| field_error(&message_text(ctx, error))),
      )
      .into()
  }

  /// One answer's button: it gives the answer the dialog's way, and says so while it is under way.
  fn answer(
    &self,
    ctx: &Ctx,
    (card, action, task): (&ApprovalCard, &DecisionAction, Option<TaskId>),
    (answer, busy, offered): (CardAnswer, bool, bool),
    full_width: bool,
  ) -> Element {
    let (id, glyph, idle, under_way) = answer.look();
    let label = ctx.t(if busy && self.requested.get() == Some(answer) {
      under_way
    } else {
      idle
    });
    let (action, requested) = (action.clone(), self.requested.clone());
    let decision = ApprovalDecision::of(&card.request, task, answer.answer());
    let button = ActionButton::new(label)
      .id(id)
      .icon(glyph)
      .compact()
      .on_click(move || {
        requested.set(Some(answer));
        action.run(decision.clone());
      })
      .enabled(!busy && offered);
    let button = if answer == CardAnswer::Once {
      button.primary()
    } else {
      button
    };
    let button = if full_width { button.fill_width() } else { button };
    button.render(ctx)
  }
}

/// *Always allow for this profile…* (a command) or *… this server…* (an MCP tool), a text action
/// (`cMk72`): with `queue` it opens the dialog's Always allow step on request `id`; without, it is
/// unavailable.
fn always_action(ctx: &Ctx, id: &str, command: bool, queue: Option<ApprovalQueue>) -> Element {
  let label = ctx.t(if command {
    "runs.conversation.card.alwaysCommand"
  } else {
    "runs.conversation.card.alwaysTool"
  });
  let content = Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(Icon::BookmarkPlus, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  let button = Button::empty()
    .button()
    .id("conversation-approval-always")
    .height(Dimension::Px(ANSWER_HEIGHT))
    .align_items(Alignment::Center)
    .child(content);
  let Some(queue) = queue else {
    return button
      .focusable(false)
      .opacity(DISABLED_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("state", "unavailable")
      .into();
  };
  let id: Arc<str> = id.into();
  keyboard_stop(button, Ring::Outer)
    .cursor(CursorIcon::Pointer)
    .describe("state", "enabled")
    .on_click(move |_| queue.review_always(&id))
    .into()
}
