//! The cards that wait for or record a person's decision, in the asking agent's message (`Cards and rows
//! inside the conversation` `YhiGC`): a question, a plan, a tool request, a question with suggested answers.
//! They are C3's (`screens/task/cards`), which plugs in here, in [`card`]. Until then each shows who asks
//! what and where it stands, and says that answering in the chat is not available yet.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{PlanDecision, ToolApprovalDecision};
use orchester_workspace::{
  display,
  tasks::{
    conversation::{Author, EntryState},
    timeline::{self, Item, ItemKind},
  },
};

use super::{
  Timeline,
  message::{Avatar, chat_row, text},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    unavailable::reason_line,
  },
};

/// Who asks what, in which step, and where it stands.
struct Asked {
  agent: Arc<str>,
  step: Option<Arc<str>>,
  title: Arc<str>,
  /// The key of where it stands.
  state: &'static str,
  /// What the agent asks, when the card says it.
  said: Option<Arc<str>>,
}

/// The card of `item`, a question, a plan, a tool request or a question with suggested answers.
pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, item: &Item) -> Element {
  let Some(Asked {
    agent,
    step,
    title,
    state,
    said,
  }) = asked(ctx, item)
  else {
    return Column::new().into();
  };
  let waits = timeline::waits_for_you(item);
  let time = display::clock(item.at);
  // The step's number and name, or its name once the run's pipeline has no such step.
  let step = timeline.step_of(ctx, item.execution).or(step);
  let meta = match &step {
    Some(step) => ctx.t_args("conversation.agentMeta", [("step", &**step), ("time", time.as_str())]),
    None => time.as_str().into(),
  };
  let said = said.map(|said| text(ctx, &format!("conversation-card-text-{}", item.key), &said));
  let frame = Column::new()
    .id(format!("conversation-card-{}", item.key))
    .describe("waits", waits.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(
      BorderSize::Sm,
      if waits {
        PaletteColor::Warning
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(RadiusSize::Lg)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Md, PaletteColor::TextSecondary))
        .child(
          Text::new(&title)
            .variant(lurq::app::theme::TypographyStyle::Label)
            .nowrap(),
        )
        .child(Spacer::new().flex(1.0))
        .child(Text::new(&ctx.t(state)).variant(ExtraTypography::Note.role()).nowrap()),
    )
    .with_children(said)
    .with_children(waits.then(|| {
      reason_line(
        &format!("conversation-card-unavailable-{}", item.key),
        &ctx.t("conversation.card.unavailable"),
      )
    }));
  chat_row(
    format!("conversation-asks-{}", item.key),
    Avatar::Agent,
    (&agent, &meta),
    vec![frame.into()],
  )
}

/// What `item`'s card says; `None` for an item that is no card.
fn asked(ctx: &Ctx, item: &Item) -> Option<Asked> {
  Some(match &item.kind {
    ItemKind::Question { entry, step } => {
      let agent = match &entry.author {
        Author::Agent(agent) | Author::YouTo(agent) => agent.clone(),
        Author::You => "".into(),
      };
      let open = entry.state == EntryState::Question { open: true };
      let title = ctx.t_args("conversation.card.asks", [("agent", &*agent)]);
      Asked {
        agent,
        step: step.clone(),
        title,
        state: answer_state(open),
        said: Some(entry.body.clone()),
      }
    }
    ItemKind::Choice(card) => {
      let title = ctx.t_args("conversation.card.asks", [("agent", &*card.agent)]);
      let said = card
        .asked
        .request
        .questions()
        .first()
        .map(|question| question.text().into());
      Asked {
        agent: card.agent.clone(),
        step: Some(card.step.clone()),
        title,
        state: answer_state(card.waiting()),
        said,
      }
    }
    ItemKind::Plan(card) => {
      let version = card.plan.version.to_string();
      let title = ctx.t_args("conversation.card.plan", [("version", version.as_str())]);
      let state = match card.plan.decision() {
        _ if card.waiting => "conversation.card.waitingApproval",
        Some(PlanDecision::Approved) => "conversation.card.approved",
        Some(PlanDecision::ChangesRequested { .. }) => "conversation.card.changesRequested",
        None => "conversation.card.superseded",
      };
      Asked {
        agent: card.agent.clone(),
        step: Some(card.step.clone()),
        title,
        state,
        said: None,
      }
    }
    ItemKind::Approval(card) => {
      let agent: Arc<str> = card.request.agent.as_str().into();
      let key = if card.request.command {
        "conversation.card.asksCommand"
      } else {
        "conversation.card.asksTool"
      };
      let title = ctx.t_args(key, [("agent", &*agent)]);
      let state = match card.decided {
        None => "conversation.card.waitingApproval",
        Some(Some(ToolApprovalDecision::Approved)) => "conversation.card.approved",
        Some(Some(ToolApprovalDecision::Denied)) => "conversation.card.denied",
        Some(None) => "conversation.card.ended",
      };
      Asked {
        agent,
        step: Some(card.step.clone()),
        title,
        state,
        said: Some(card.request.action.as_str().into()),
      }
    }
    _ => return None,
  })
}

const fn answer_state(open: bool) -> &'static str {
  if open {
    "conversation.card.waitingAnswer"
  } else {
    "conversation.card.answered"
  }
}
