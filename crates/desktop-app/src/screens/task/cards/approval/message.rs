//! *Deny with a message* (`Deny message` `YsB30`, `Answers` `zSxlY` and `Keys` `xIRh8` in `D2fdTQ`): in place of
//! the answers, *Message to Builder* in 11.5 `text_muted`, a text area 64 high, and what the agent is told in 11
//! `text_muted`, 6 apart; then *Back* and *Deny and send message* at the right, and which keys do the same. The agent
//! gets the words as the denial's reason; Codex, whose decline carries no words, declines in its own and gets them as
//! the next message of its turn, which the card says. *Deny and send message* waits for words, at most
//! [`DenialNote::MAX_CHARS`] of them; Ctrl+Enter (⌘ Enter on macOS) in the field sends, Esc there goes back.

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  orchester_tools,
  run::{
    Answerer, ApprovalAnswer, DenialNote, DenialNoteError, PendingToolApproval, denial_message, denial_with_message,
  },
};
use orchester_ipc::Command;
use orchester_workspace::workspace::OpenProject;

use super::{
  super::{
    decisions::Given,
    frame::{held, note},
  },
  answers::Answering,
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    icon::Icon,
  },
};

/// The message field's least height (`zvvog` in `YsB30`).
const FIELD_HEIGHT: f32 = 64.0;

/// What the card shows while the person writes their message: the message, the answers and the keys, each a part of
/// the card.
pub(super) fn editor(ctx: &mut Ctx, answering: &Answering<'_>) -> Vec<Element> {
  let id = answering.id;
  let written = answering.message.get();
  let note = DenialNote::new(&written);
  let open = answering.hold(Given::DenyWithMessage);
  let send = match (&note, open.clone()) {
    (Ok(_), Ok(open)) => Some(answering.send_message(open)),
    _ => None,
  };
  let block = Column::new()
    .id(format!("{id}-message-block"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(note_label(ctx, answering))
    .child(field(ctx, answering, note.as_ref().err()))
    .child(told(ctx, answering, note.as_ref().ok()))
    .on_key_down(keys(answering, send.clone()));
  let back = {
    let button = ActionButton::new(
      format!("{id}-message-back"),
      ButtonKind::Ghost,
      ctx.t("cards.approval.message.back"),
    )
    .in_form();
    match open {
      Err(true) => button.disabled(),
      _ => {
        let editing = answering.editing.clone();
        button.on_click(move || editing.set(false))
      }
    }
  };
  let sending = ActionButton::new(
    format!("{id}-message-send"),
    ButtonKind::Secondary,
    ctx.t("cards.approval.message.send"),
  )
  .icon(Icon::Send)
  .in_form();
  let sending = match (open, send) {
    (Err(true), _) => sending.loading(),
    (_, Some(send)) => sending.on_click(send),
    (_, None) => sending.disabled(),
  };
  let row = Row::new()
    .id(format!("{id}-message-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(back)
    .child(sending);
  let keys = Text::new(&ctx.t_args("cards.approval.message.keys", [("keys", &*send_keys(ctx))]))
    .id(format!("{id}-message-keys"))
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0));
  let mut parts: Vec<Element> = vec![block.into(), row.into()];
  if answering.decisions.busy() && answering.sending().is_none() {
    parts.push(held(ctx, id));
  }
  parts.push(keys.into());
  parts
}

/// *Message to Builder*.
fn note_label(ctx: &Ctx, answering: &Answering<'_>) -> Text {
  note(
    &format!("{}-message-label", answering.id),
    &ctx.t_args(
      "cards.approval.message.label",
      [("agent", answering.request.agent.as_str())],
    ),
  )
}

/// The text area the message is written in, which takes the focus as it opens; with why it cannot be sent when it
/// is too long.
fn field(ctx: &mut Ctx, answering: &Answering<'_>, refused: Option<&DenialNoteError>) -> Element {
  let input = FieldInput::new(
    format!("{}-message", answering.id),
    ctx.t("cards.approval.message.field"),
    answering.message.clone(),
  )
  .text_area()
  .area_height(FIELD_HEIGHT)
  .element(answering.field.clone());
  let input = match refused {
    Some(DenialNoteError::TooLong { chars }) => input.error(ctx.t_args(
      "cards.approval.message.tooLong",
      [
        ("max", DenialNote::MAX_CHARS.to_string().as_str()),
        ("count", chars.to_string().as_str()),
      ],
    )),
    Some(DenialNoteError::Empty) | None => input,
  };
  input.render(ctx)
}

/// What the agent is told: the denial and the words, as its CLI gets them.
fn told(ctx: &Ctx, answering: &Answering<'_>, note: Option<&DenialNote>) -> Text {
  let request = answering.request;
  let message = told_words(request, note);
  let agent = request.agent.as_str();
  let key = if codex_declines(answering) {
    "cards.approval.message.toldCodex"
  } else {
    "cards.approval.message.told"
  };
  Text::new(&ctx.t_args(key, [("agent", agent), ("message", message.as_str())]))
    .id(format!("{}-message-told", answering.id))
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0))
}

/// The words the agent gets: the denial with the message, or the plain denial while none is written.
fn told_words(request: &PendingToolApproval, note: Option<&DenialNote>) -> String {
  match note {
    Some(note) => denial_with_message(request, Answerer::Person, note),
    None => denial_message(request, Answerer::Person),
  }
}

/// Codex asks, and declines in its own words: a call of Orchester's tools is refused with the message itself.
fn codex_declines(answering: &Answering<'_>) -> bool {
  answering.shown.is_some_and(|shown| shown.choices.codex) && answering.request.server != orchester_tools::SERVER
}

/// The keys that deny and send, as this platform names them.
fn send_keys(ctx: &Ctx) -> std::sync::Arc<str> {
  ctx.t(if cfg!(target_os = "macos") {
    "cards.approval.message.cmdEnter"
  } else {
    "cards.approval.message.ctrlEnter"
  })
}

/// In the field, Ctrl+Enter (or ⌘ Enter) sends while it can be sent and Esc goes back to the answers.
fn keys(
  answering: &Answering<'_>,
  send: Option<impl Fn() + Send + Sync + 'static>,
) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let (field, editing) = (answering.field.clone(), answering.editing.clone());
  move |event: KeyboardEvent| {
    if !field.focused() {
      return;
    }
    if event.key == "Escape" {
      event.prevent_default();
      editing.set(false);
      return;
    }
    if event.key != "Enter" || !(event.ctrl || event.meta) {
      return;
    }
    event.prevent_default();
    if event.composing {
      return;
    }
    if let Some(send) = &send {
      send();
    }
  }
}

impl Answering<'_> {
  /// Sends the written message as a denial's words, by whoever's input pressed it ([`super::super::decisions::
  /// Decisions::answer`]); nothing while the words cannot be sent.
  pub(super) fn send_message(&self, open: OpenProject) -> impl Fn() + Clone + Send + Sync + 'static {
    let (decisions, (run, key)) = (self.decisions.clone(), (self.card.0, self.card.1.clone()));
    let (request, approval) = (self.request.run, self.request.id.clone());
    let message = self.message.clone();
    move || {
      let Ok(note) = DenialNote::new(&message.get_untracked()) else {
        return;
      };
      let command = |by| Command::DecideToolApproval {
        run: request,
        approval: approval.clone(),
        answer: ApprovalAnswer::DenyWithMessage { message: note },
        by,
      };
      decisions.answer((run, &key), Given::DenyWithMessage, (&open, command));
    }
  }
}
