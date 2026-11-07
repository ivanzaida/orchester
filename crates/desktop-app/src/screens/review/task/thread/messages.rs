//! A thread's messages (`Message` `csUdM` in `RBR9O`), oldest first, 8 over and under and 14 at their sides, a
//! `border` line between them: who wrote it (the person's round 22 avatar and *You*, or Implement's 24 tile and
//! *Implement · Builder*) with when or what it does now, and its words in 13, 1.45, 30 in from the avatar's start.
//! While Implement starts, its row says *Starting…* over three dots (`Message · waiting` `sXET0`); while it writes, its
//! live activity in `success` and its words with a caret after them; then *Stop answering* with *Esc* under it. A sent
//! comment links to the Conversation where Implement works on it; a stopped answer ends in *Answer stopped.*
//! Orchester's own message, on `warning_muted`, says why a question was not asked or a comment not sent (`Message ·
//! Orchester` `gglnp`).

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
use orchester_core::review::{AgentWords, CommentState, ReviewMessage, Said, ThreadState};
use orchester_workspace::{
  display::clock,
  tasks::review_threads::{failure_message, read::ThreadView},
};

use super::{
  super::{
    Env,
    actions::Write,
    composer::in_thread_box,
    state::{Held, InThread},
    words::agent,
  },
  look::words_arrived,
};
use crate::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraRadius, ExtraSpacing, IconSize, REVIEW_TYPING_OPACITY, ReviewSpacing, ReviewText},
  ui::{
    avatar::{agent_avatar, initials, person_avatar},
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};

/// A typing dot and the caret after words being written (`Dot` `h6EkpB`, `Caret` `ywXbg`).
const DOT_SIZE: f32 = 6.0;
const CARET_WIDTH: f32 = 2.0;
const CARET_HEIGHT: f32 = 14.0;

/// Who wrote a message, and what its row says beside the name.
struct Who {
  avatar: Row,
  name: Arc<str>,
  meta: Arc<str>,
  meta_color: PaletteColor,
}

/// The messages of `view`, and while Implement starts or writes, its row and *Stop answering*.
pub(super) fn messages(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Vec<Element> {
  let id = format!("review-thread-{}", view.thread.id());
  let stopped = view.state == ThreadState::AnswerStopped;
  let all = view.thread.messages();
  let mut shown: Vec<Element> = Vec::new();
  for (at, message) in all.iter().enumerate() {
    let message_id = format!("{id}-message-{at}");
    let last = at + 1 == all.len();
    let asked_before = all[..at]
      .iter()
      .rev()
      .find(|earlier| earlier.said().is_person())
      .is_some_and(|earlier| matches!(earlier.said(), Said::Question(_)));
    shown.push(match message.said() {
      Said::Agent(words) => agent_message(ctx, env, (&message_id, message, words), (last && stopped, asked_before)),
      _ => person_message(ctx, env, (&message_id, message), view),
    });
  }
  let answering = !env.read_only && matches!(view.state, ThreadState::Asked | ThreadState::Answering);
  if answering && !words_arrived(view) && !writing(all) {
    shown.push(starting(ctx, env, &id));
  }
  if answering {
    shown.push(stop_row(ctx, env, (&id, view)));
  }
  if let ThreadState::NotAsked(failure) | ThreadState::CouldNotSend(failure) = &view.state {
    let reason = message_text(ctx, &failure_message(failure));
    shown.push(orchester(ctx, env, (&id, view), &reason));
  }
  shown
}

/// Whether Implement's latest words are being written.
fn writing(messages: &[ReviewMessage]) -> bool {
  messages
    .last()
    .is_some_and(|message| matches!(message.said(), Said::Agent(AgentWords { writing: true, .. })))
}

/// A message's frame: who wrote it over `body`, past the avatar; on `tint` when it has one.
fn frame(id: &str, who: Who, body: Vec<Element>, tint: Option<PaletteColor>) -> Element {
  let row = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(who.avatar)
    .child(
      Text::new(&who.name)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&who.meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Description)
        .color(who.meta_color)
        .nowrap(),
    );
  let column = Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(row)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(ReviewSpacing::BodyIndent.role()))
        .spacing(ExtraSpacing::Snug.role())
        .with_children(body),
    );
  match tint {
    Some(tint) => column.background(tint).into(),
    None => column.into(),
  }
}

/// Words of a message, wrapping.
fn words(id: &str, text: &str) -> Element {
  Text::new(text)
    .id(id.to_owned())
    .variant(ReviewText::Message.role())
    .width(Dimension::Pct(100.0))
    .into()
}

fn you(ctx: &Ctx, id: &str, meta: Arc<str>) -> Who {
  let you = ctx.t("review_page.thread.you");
  Who {
    avatar: person_avatar(&format!("{id}-avatar"), &initials(&you)),
    name: you,
    meta,
    meta_color: PaletteColor::TextMuted,
  }
}

fn implement(ctx: &Ctx, env: &Env, id: &str, (meta, meta_color): (Arc<str>, PaletteColor)) -> Who {
  Who {
    avatar: agent_avatar(&format!("{id}-avatar"), Icon::Bot),
    name: agent(ctx, &env.page),
    meta,
    meta_color,
  }
}

/// The person's message: a question asked, or a comment with when it was written or sent; its words, or the box that
/// changes them while it is edited; a sent comment's link to the Conversation.
fn person_message(ctx: &mut Ctx, env: &Env, (id, message): (&str, &ReviewMessage), view: &ThreadView) -> Element {
  let time = clock(message.at());
  let meta = match message.said() {
    Said::Question(_) => ctx.t_args("review_page.thread.asked", [("time", time)]),
    Said::Comment(CommentState::Sent { at }) => ctx.t_args("review_page.thread.sentAt", [("time", clock(*at))]),
    _ => time.into(),
  };
  let editing = env.state.in_thread.get() == Held(Some(InThread::Edit(message.id())));
  let mut body = vec![if editing {
    in_thread_box(ctx, env, view, InThread::Edit(message.id()))
  } else {
    words(&format!("{id}-body"), message.body())
  }];
  if matches!(message.said(), Said::Comment(CommentState::Sent { .. })) {
    body.push(conversation_link(ctx, env, id));
  }
  frame(id, you(ctx, id, meta), body, None)
}

/// *In the Conversation · run #4, step 03 Implement ↗*: the task's Conversation, where Implement works on the comment;
/// *In the Conversation · run #4 ↗* while the implementing step is not known.
fn conversation_link(ctx: &Ctx, env: &Env, id: &str) -> Element {
  let run = ("run", env.page.run.number.get().to_string());
  let words = match &env.page.implement {
    Some(implement) => ctx.t_args(
      "review_page.thread.inConversation",
      [
        run,
        ("number", format!("{:02}", implement.number)),
        ("step", implement.name.to_string()),
      ],
    ),
    None => ctx.t_args("review_page.thread.inConversationRun", [run]),
  };
  let (navigation, route) = (
    env.navigation.clone(),
    Route::Project(
      env.project.open.project.id(),
      ProjectPage::Task(env.page.run.key.clone(), TaskView::of(TaskTab::Conversation)),
    ),
  );
  pressable(move || navigation.go(route.clone()))
    .id(format!("{id}-conversation"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::MessagesSquare, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// Implement's words: while it writes, its activity and the words with a caret (three dots before the first); once
/// written, when it answered or stopped, with *Answer stopped.* under a stopped answer.
fn agent_message(
  ctx: &Ctx,
  env: &Env,
  (id, message, said): (&str, &ReviewMessage, &AgentWords),
  (stopped, answered): (bool, bool),
) -> Element {
  let time = clock(message.at());
  let empty = message.body().trim().is_empty();
  let meta = match (said.writing, &said.activity) {
    (true, Some(activity)) => (Arc::from(activity.as_str()), PaletteColor::Success),
    (true, None) if empty => (ctx.t("review_page.thread.starting"), PaletteColor::TextMuted),
    (true, None) => (ctx.t("review_page.thread.writing"), PaletteColor::Success),
    (false, _) if stopped => (
      ctx.t_args("review_page.thread.stoppedAt", [("time", time)]),
      PaletteColor::TextMuted,
    ),
    (false, _) if answered => (
      ctx.t_args("review_page.thread.answeredAt", [("time", time)]),
      PaletteColor::TextMuted,
    ),
    (false, _) => (time.into(), PaletteColor::TextMuted),
  };
  let body: Vec<Element> = match (said.writing, empty) {
    (true, true) => vec![dots(id)],
    (true, false) => vec![streaming(id, message.body())],
    (false, _) if stopped => vec![
      words(&format!("{id}-body"), message.body()),
      Text::new(&ctx.t("review_page.thread.answerStopped"))
        .id(format!("{id}-stopped"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .into(),
    ],
    (false, _) => vec![words(&format!("{id}-body"), message.body())],
  };
  frame(id, implement(ctx, env, id, meta), body, None)
}

/// Implement's row before anything arrived: *Starting…* over three dots.
fn starting(ctx: &Ctx, env: &Env, id: &str) -> Element {
  let id = format!("{id}-starting");
  let meta = (ctx.t("review_page.thread.starting"), PaletteColor::TextMuted);
  frame(&id, implement(ctx, env, &id, meta), vec![dots(&id)], None)
}

/// Three dots, fading, while nothing was written yet.
fn dots(id: &str) -> Element {
  Row::new()
    .id(format!("{id}-dots"))
    .padding(Padding::new().top(SpacingSize::Xs).bottom(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .with_children(REVIEW_TYPING_OPACITY.map(|opacity| -> Element {
      Row::new()
        .width(Dimension::Px(DOT_SIZE))
        .height(Dimension::Px(DOT_SIZE))
        .rounded(ExtraRadius::Full.role())
        .background(PaletteColor::TextMuted)
        .opacity(opacity)
        .into()
    }))
    .into()
}

/// Words being written, the caret after them.
fn streaming(id: &str, text: &str) -> Element {
  Row::new()
    .id(format!("{id}-streaming"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::End)
    .child(
      Text::new(text)
        .id(format!("{id}-body"))
        .variant(ReviewText::Message.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Row::new()
        .id(format!("{id}-caret"))
        .width(Dimension::Px(CARET_WIDTH))
        .height(Dimension::Px(CARET_HEIGHT))
        .background(PaletteColor::Success),
    )
    .into()
}

/// *Stop answering* and its key, under Implement's row.
fn stop_row(ctx: &Ctx, env: &Env, (id, view): (&str, &ThreadView)) -> Element {
  let (writes, thread) = (env.writes.clone(), view.thread.id());
  let stop = ActionButton::new(
    format!("{id}-stop"),
    ButtonKind::Secondary,
    ctx.t("review_page.thread.stop"),
  )
  .icon(Icon::CircleStop)
  .snug()
  .on_click(move || writes.run(Write::Stop(thread)));
  Row::new()
    .id(format!("{id}-stop-row"))
    .padding(
      Padding::new()
        .left(ReviewSpacing::BodyIndent.role())
        .right(ExtraSpacing::Inset.role())
        .bottom(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(stop)
    .child(
      Text::new(&ctx.t("review_page.thread.stopKey"))
        .variant(crate::theme::ExtraTypography::MonoNote.role())
        .nowrap(),
    )
    .into()
}

/// Orchester's message: why the question was not asked, with *Ask again*, or why the comment was not sent.
fn orchester(ctx: &Ctx, env: &Env, (id, view): (&str, &ThreadView), reason: &str) -> Element {
  let id = format!("{id}-orchester");
  let who = Who {
    avatar: agent_avatar(&format!("{id}-avatar"), Icon::CircleAlert),
    name: ctx.t("review_page.thread.orchester"),
    meta: clock(view.thread.latest_ask().at()).into(),
    meta_color: PaletteColor::TextMuted,
  };
  let mut body = vec![words(&format!("{id}-body"), reason)];
  if let ThreadState::NotAsked(_) = view.state {
    let ask = super::ask_again(ctx, env, view, (&format!("{id}-ask-again"), ButtonKind::Primary));
    body.push(Row::new().child(ask).into());
  }
  frame(&id, who, body, Some(PaletteColor::WarningMuted))
}
