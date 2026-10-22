//! A reply in a thread (`Reply composer` `T6s2wF`): what it asks (a question changes no code, a fix request does),
//! that it stays in Orchester, the words, and *Send reply*; Esc closes it and keeps the draft, Ctrl+Enter (Cmd+Enter)
//! sends. It goes with the thread's context, at the pull request's head.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::pull_request::feedback::FeedbackKind;
use orchester_workspace::pull_requests::feedback::ThreadView;

use super::{
  Composing, Write,
  composer::{Press, keys, send_button, send_keys, words_box},
  parts::visibility,
  thread::Env,
  words,
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    kbd::key_cap,
  },
};

/// A reply in a thread.
#[derive(Clone)]
pub struct ReplyProps {
  pub env: Env,
  pub thread: ThreadView,
}

pub fn reply(ctx: &mut Ctx, props: &ReplyProps) -> Element {
  let (env, thread) = (&props.env, &props.thread);
  let Some(task) = env.board.task.clone() else {
    return Element::new();
  };
  let id = thread.id;
  let composing = Composing::Reply(id);
  let draft = env.state.drafts.of(&composing, thread.kind);
  let kind = draft.kind.get();
  let ready = !draft.text.get().trim().is_empty() && env.head.is_some() && !env.writes.busy();
  let press: Option<Press> = ready.then(|| {
    let (writes, draft, head) = (env.writes.clone(), draft.clone(), env.head.clone());
    Arc::new(move || {
      let Some(revision) = head.clone() else {
        return;
      };
      writes.run(
        Write::Reply {
          key: draft.key,
          thread: id,
          kind: draft.kind.get_untracked(),
          text: draft.text.get_untracked(),
          revision,
        },
        Some(Composing::Reply(id)),
      );
    }) as Press
  });
  let mode = match kind {
    FeedbackKind::Question => ctx.t("pull_requests.feedback.composer.replyQuestion"),
    FeedbackKind::Fix => ctx.t("pull_requests.feedback.composer.replyFix"),
  };
  let send = send_button(
    ActionButton::new(
      format!("feedback-{id}-send-reply"),
      ButtonKind::Primary,
      ctx.t("pull_requests.feedback.composer.sendReply"),
    )
    .icon(Icon::Send),
    press.as_ref(),
    env.writes.busy(),
  );
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(words::kind_icon(kind), IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&mode).variant(ExtraTypography::Note.role()).nowrap())
    .child(Row::new().flex(1.0))
    .child(visibility(ctx, &format!("feedback-{id}-reply-visibility"), false));
  let foot = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(key_cap(
      &format!("feedback-{id}-reply-esc"),
      &ctx.t("pull_requests.feedback.composer.esc"),
    ))
    .child(
      Text::new(&ctx.t("pull_requests.feedback.composer.escape"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(send_keys(ctx, &format!("feedback-{id}-reply-keys"), press.is_some()))
    .child(send);
  Column::new()
    .id(format!("feedback-{id}-reply"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .on_key_down(keys(&env.state, press))
    .child(head)
    .child(words_box(ctx, &format!("feedback-{id}-reply-text"), (&task, &draft)))
    .child(foot)
    .into()
}
