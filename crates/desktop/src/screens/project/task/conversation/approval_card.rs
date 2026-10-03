//! A tool request inline in the timeline (`sACJF`, `pSVDW`): who asks for what and, while it waits, the
//! approval dialog's answers in one row (`KpZxA`): *Always allow…* at the left, then *Deny*, *Approve
//! similar in this run* and *Approve once*. Each answer goes the dialog's own way
//! ([`ApprovalDecision::of`], the same action and runner command); *Always allow…* opens the dialog's
//! Always allow step on this request. What the request offers is the dialog's too: an answer it does
//! not offer stays at 0.4, and why is said under the row in the dialog's words.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, dimension::Dimension},
};
use orchester_core::run::{ApprovalAnswer, SimilarUnavailable, ToolApprovalDecision};

use super::{
  super::tool_approval::{
    Decision as ApprovalDecision, DecisionAction, always_blocked, decision_action as approval_decision,
    similar_unavailable,
  },
  cards::{frame, header},
};
use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, pages::Shown},
  tasks::{
    approvals::{AlwaysBlocked, ShownApproval},
    timeline::ApprovalCard,
  },
  theme::DISABLED_OPACITY,
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
    property::{ValueStyle, styled_property},
  },
};

/// The answers' height (`KpZxA`).
const ANSWER_HEIGHT: f32 = 30.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApprovalCardProps {
  pub(super) card: ApprovalCard,
}

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

pub(super) struct ApprovalCardView {
  requested: Signal<Option<CardAnswer>>,
}

impl Component for ApprovalCardView {
  type Props = ApprovalCardProps;

  fn create(ctx: &mut Ctx) -> Self {
    approval_decision(ctx);
    Self {
      requested: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ApprovalCardProps>().clone();
    let action = approval_decision(ctx);
    let request = &props.card.request;
    let waiting = props.card.decided.is_none();
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
    let state = match props.card.decided {
      None => (Icon::Hand, ctx.t("runs.conversation.state.waiting")),
      Some(Some(ToolApprovalDecision::Approved)) => (Icon::Check, ctx.t("runs.conversation.card.approved")),
      Some(Some(ToolApprovalDecision::Denied)) => (Icon::X, ctx.t("runs.conversation.card.denied")),
      Some(None) => (Icon::CirclePause, ctx.t("runs.conversation.card.interrupted")),
    };
    let mut body: Vec<Element> = Vec::new();
    if !request.command {
      body.push(styled_property(
        &ctx.t("runs.toolApproval.server"),
        &request.server,
        ValueStyle::Mono,
      ));
    }
    body.push(styled_property(
      &ctx.t(if request.command {
        "runs.toolApproval.command"
      } else {
        "runs.toolApproval.action"
      }),
      &request.action,
      ValueStyle::Mono,
    ));
    if let Some(target) = request.target.as_deref() {
      body.push(styled_property(
        &ctx.t("runs.toolApproval.target"),
        target,
        ValueStyle::Mono,
      ));
    }
    if waiting {
      let shown = shown_request(ctx, &request.id);
      let queue = ctx.use_context::<ApprovalQueue>();
      body.push(self.answers(ctx, (&props.card, shown.as_ref()), &action, queue));
    }
    frame(
      format!("conversation-approval-{}", request.id),
      waiting,
      header(Icon::ShieldAlert, &title, &meta, state),
      body,
    )
  }
}

/// The request with its choices, as the project's waiting requests know it.
fn shown_request(ctx: &mut Ctx, id: &str) -> Option<ShownApproval> {
  let shown = ctx.use_context::<Shown>()?;
  let tasks = shown.tasks.ready()?;
  tasks
    .pending_tool_approvals
    .iter()
    .find(|pending| pending.approval.id == id)
    .cloned()
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

  /// Why an answer is unavailable, in the dialog's words.
  fn reasons(&self, ctx: &Ctx) -> Vec<Arc<str>> {
    [
      self
        .similar
        .and_then(Result::err)
        .map(|unavailable| ctx.t(similar_unavailable(unavailable))),
      self
        .always
        .and_then(Result::err)
        .map(|blocked| always_blocked(ctx, blocked)),
    ]
    .into_iter()
    .flatten()
    .collect()
  }
}

impl ApprovalCardView {
  /// The row of answers, why one is unavailable, and why an answer was not carried out.
  fn answers(
    &self,
    ctx: &Ctx,
    (card, shown): (&ApprovalCard, Option<&ShownApproval>),
    action: &DecisionAction,
    queue: Option<ApprovalQueue>,
  ) -> Element {
    let state = action.state().get();
    let busy = state.is_pending() || state.data.is_some();
    let offers = Offers::of(shown);
    let button = |answer: CardAnswer, offered: bool| self.answer(ctx, (card, action), (answer, busy, offered));
    let always = queue.filter(|_| matches!(offers.always, Some(Ok(()))) && !busy);
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(always_action(ctx, &card.request.id, card.request.command, always))
      .child(Spacer::new().flex(1.0))
      .child(button(CardAnswer::Deny, true))
      .child(button(CardAnswer::Similar, matches!(offers.similar, Some(Ok(())))))
      .child(button(CardAnswer::Once, true));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(row)
      .with_children(offers.reasons(ctx).into_iter().map(|reason| {
        Text::new(&reason)
          .id("conversation-approval-unavailable")
          .variant(TypographyStyle::Caption)
      }))
      .with_children(
        state
          .error
          .filter(|_| !busy)
          .map(|error| field_error(&message_text(ctx, &error))),
      )
      .into()
  }

  /// One answer's button: it gives the answer the dialog's way, and says so while it is under way.
  fn answer(
    &self,
    ctx: &Ctx,
    (card, action): (&ApprovalCard, &DecisionAction),
    (answer, busy, offered): (CardAnswer, bool, bool),
  ) -> Element {
    let (id, glyph, idle, under_way) = answer.look();
    let label = ctx.t(if busy && self.requested.get() == Some(answer) {
      under_way
    } else {
      idle
    });
    let (action, requested) = (action.clone(), self.requested.clone());
    let decision = ApprovalDecision::of(&card.request, answer.answer());
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
