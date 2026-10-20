//! An answer or a message the project's Orchestrator chat proposed for the run and no one applied (ORC-45), after the
//! timeline's rows. While it waits: the Orchestrator's row (its `waypoints` avatar, its name and when it proposed it),
//! what it would send (for a message that moves the run, how, in the words of the chat's card, `ExRt1`), *Waiting
//! for you in the Orchestrator chat*, and the card's action (*Send message*, *Run again with this message*, *Send
//! answer*, ...), *Dismiss* and *Open in Orchestrator chat*. The action and *Dismiss* decide the chat's own proposal,
//! as its card does, so deciding in either place decides both; only a person applies it: lurq's synthetic input is
//! refused in the handler, visibly, and *Dismiss* stays open to automation, as on the card. Once dismissed, refused or
//! stale, it leaves a muted line that says nothing was sent. Which proposals there are and where each stands are
//! `orchester_workspace::tasks::conversation::orchestrator`'s.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  display,
  orchestrator::{proposal::Decision, tools::ChatScope},
  tasks::conversation::orchestrator::{Proposal, ProposalState, Proposed},
};

use super::super::super::{
  super::orchestrator::{
    card::{DecideAction, Deciding, decide_action},
    change::answer::moving,
  },
  orchestrated::open_chat,
};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    chat::{Avatar, chat_row, delivery_line},
    icon::{Icon, icon},
    message::message_text,
    tinted_note::tinted_note,
  },
};

/// The project whose chat holds the proposal: its id, its name and its folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct ChatProject {
  pub(in super::super) id: ProjectId,
  pub(in super::super) name: Arc<str>,
  pub(in super::super) root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProposalProps {
  pub(super) project: ChatProject,
  pub(super) proposal: Proposal,
}

impl DevtoolsInspectable for ProposalProps {}

pub(super) struct ProposalRow {
  decide: DecideAction,
  /// Synthetic input tried *Apply*.
  refused: Signal<bool>,
}

impl Component for ProposalRow {
  type Props = ProposalProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      decide: decide_action(ctx),
      refused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ProposalProps>().clone();
    let id = format!("conversation-{}", props.proposal.key());
    let shown = match &props.proposal.state {
      ProposalState::Waits | ProposalState::Applying => self.pending(ctx, &id, &props),
      ProposalState::Dismissed => settled(&id, &ctx.t("conversation.orchestrator.proposal.dismissed")),
      ProposalState::Stale => settled(&id, &ctx.t("conversation.orchestrator.proposal.stale")),
      ProposalState::Refused(reason) => settled(
        &id,
        &ctx.t_args("conversation.orchestrator.proposal.refused", [("reason", &**reason)]),
      ),
    };
    shown.describe("proposal", state_name(&props.proposal.state))
  }
}

impl ProposalRow {
  /// The Orchestrator's row while the proposal waits or is being applied.
  fn pending(&self, ctx: &mut Ctx, id: &str, props: &ProposalProps) -> Column {
    let proposal = &props.proposal;
    let name = ctx.t("conversation.byOrchestrator");
    let time = display::clock(proposal.at);
    let (glyph, words) = match proposal.state {
      ProposalState::Applying => (Icon::LoaderCircle, ctx.t("conversation.orchestrator.proposal.applying")),
      _ => (Icon::Hand, ctx.t("conversation.orchestrator.proposal.waits")),
    };
    let mut body = said(ctx, id, &proposal.proposed);
    body.push(delivery_line(
      &format!("{id}-state"),
      (glyph, PaletteColor::TextMuted),
      &words,
    ));
    body.push(self.actions(ctx, id, props));
    body.extend(self.notes(ctx, id));
    Column::new()
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .child(chat_row(
        format!("{id}-row"),
        Avatar::Orchestrator,
        (&name, &time),
        body,
      ))
  }

  /// *Apply* and *Dismiss* while it waits, then *Open in Orchestrator chat*.
  fn actions(&self, ctx: &mut Ctx, id: &str, props: &ProposalProps) -> Element {
    let proposal = &props.proposal;
    let mut row = Row::new()
      .id(format!("{id}-actions"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    if proposal.waits() {
      let deciding = |decision| Deciding {
        scope: ChatScope {
          project: props.project.id,
          name: props.project.name.to_string(),
          root: props.project.root.clone(),
          chat: proposal.chat,
          // The person decides here, as on the chat's card.
          on_its_own: false,
        },
        seq: proposal.seq,
        decision,
      };
      // Named as the chat's card names it: what applying sends, and how it moves the run.
      let (label, glyph) = match &proposal.proposed {
        Proposed::Message { then, .. } => (moving(*then).1, Icon::MessageSquareText),
        Proposed::Answer { .. } => ("orchestrator.change.answer.accept", Icon::Reply),
      };
      let apply = ActionButton::new(format!("{id}-apply"), ButtonKind::Primary, ctx.t(label))
        .icon(glyph)
        .compact();
      let dismiss = ActionButton::new(
        format!("{id}-dismiss"),
        ButtonKind::Secondary,
        ctx.t("conversation.orchestrator.proposal.dismiss"),
      )
      .compact();
      let (apply, dismiss) = if self.decide.state().get().is_pending() {
        (apply.loading(), dismiss.disabled())
      } else {
        let (decide, refused, applying) = (
          self.decide.clone(),
          self.refused.clone(),
          deciding(Decision::Accept { widens: Vec::new() }),
        );
        let apply = apply.on_click(move || {
          if is_synthetic_input() {
            tracing::warn!(
              "synthetic input reached a proposal of the Orchestrator in a task's conversation; not taken"
            );
            refused.set(true);
            return;
          }
          decide.run(applying.clone());
        });
        let (decide, dismissing) = (self.decide.clone(), deciding(Decision::Dismiss));
        (apply, dismiss.on_click(move || decide.run(dismissing.clone())))
      };
      row = row.child(apply).child(dismiss);
    }
    row.child(open_chat(ctx, id, (props.project.id, proposal.chat))).into()
  }

  /// A refused click of synthetic input, and why the last decision failed.
  fn notes(&self, ctx: &Ctx, id: &str) -> Vec<Element> {
    let mut notes = Vec::new();
    if self.refused.get() {
      notes.push(
        tinted_note(
          format!("{id}-person-only"),
          (Icon::Hand, PaletteColor::Warning),
          &ctx.t("conversation.orchestrator.proposal.personOnly"),
        )
        .into(),
      );
    }
    if let Some(error) = self.decide.state().get().error {
      notes.push(
        tinted_note(
          format!("{id}-error"),
          (Icon::CircleAlert, PaletteColor::Danger),
          &message_text(ctx, &error),
        )
        .into(),
      );
    }
    notes
  }
}

/// What the proposal would send: the message as written, or each option the answer picks and its own words.
fn said(ctx: &Ctx, id: &str, proposed: &Proposed) -> Vec<Element> {
  let text = |suffix: String, words: &str| -> Element {
    Text::new(words)
      .id(format!("{id}-{suffix}"))
      .variant(TypographyStyle::Body)
      .selectable(true)
      .width(Dimension::Pct(100.0))
      .into()
  };
  match proposed {
    Proposed::Message {
      text: message,
      then: None,
    } => vec![text("text".to_owned(), message)],
    // One that moves a run none of whose agents waits or works says how, with its text, as the chat's card does.
    Proposed::Message { text: message, then } => vec![text(
      "text".to_owned(),
      &ctx.t_args(moving(*then).0, [("text", &**message)]),
    )],
    Proposed::Answer { choices, text: own, .. } => choices
      .iter()
      .map(|choice| ctx.t_args("conversation.orchestrator.proposal.choice", [("choice", &**choice)]))
      .chain(
        own
          .iter()
          .map(|own| ctx.t_args("conversation.orchestrator.proposal.ownWords", [("text", &**own)])),
      )
      .enumerate()
      .map(|(index, words)| text(format!("answer-{index}"), &words))
      .collect(),
  }
}

/// The muted line a proposal leaves once it no longer waits: the Orchestrator's mark and that nothing was sent.
fn settled(id: &str, words: &str) -> Column {
  Column::new().id(id.to_owned()).width(Dimension::Pct(100.0)).child(
    Row::new()
      .id(format!("{id}-settled"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Waypoints, IconSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(words).variant(TypographyStyle::Caption)),
  )
}

/// The name `describe` reports a proposal's state by, for tests and QA.
const fn state_name(state: &ProposalState) -> &'static str {
  match state {
    ProposalState::Waits => "waits",
    ProposalState::Applying => "applying",
    ProposalState::Dismissed => "dismissed",
    ProposalState::Stale => "stale",
    ProposalState::Refused(_) => "refused",
  }
}
