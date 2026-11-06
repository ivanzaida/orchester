//! A thread under its lines (`Thread · 4xx` `hxP69` in 09i, every state of `RBR9O`): 8 over it, 10 under it, 94 in
//! from the diff's start past the numbers and the sign and 16 from its end, a box on `surface_raised` in its kind's
//! stroke (a comment's `info`, a question's `success`), radius 6. Its head, 30 high on `surface_panel`, 14 at its
//! sides: the kind's glyph and word in its color, the lines in Geist Mono 12 `text_muted`, then its state's tag and
//! what can be done now (*Edit* and *Delete* while a comment is not sent, *Ask again* and *Resolve* once an answer
//! ended, *Reply* and *Resolve* once a comment was addressed). Its messages follow ([`messages`]). Outdated threads
//! fold into one line that opens them (*N outdated*), a resolved thread into a line of its own; an opened resolved
//! thread can be reopened. The comment box is drawn by [`super::composer`].

mod look;
mod messages;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  conversation::MessageBody,
  review::{Said, ThreadKind, ThreadState},
};
use orchester_workspace::tasks::review_threads::read::ThreadView;

use super::{
  Env,
  actions::{Kept, Write},
  composer::{in_thread_box, new_box},
  diff::{COMPOSER, outdated_key, resolved_key, thread_key},
  state::InThread,
  tag::tag,
  words::lines,
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ReviewSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
    pressable::pressable,
  },
};

/// A thread's head (`Head` `hW8vx`).
const HEAD_HEIGHT: f32 = 30.0;

/// The block of `key`: a thread, a fold, or the comment box.
pub(super) fn block(ctx: &mut Ctx, env: &Env, key: &Arc<str>) -> Element {
  if **key == *COMPOSER {
    return wrap(new_box(ctx, env));
  }
  if let Some(path) = key.strip_prefix("outdated:") {
    return wrap(outdated_fold(ctx, env, path));
  }
  let Some(view) = env
    .review
    .threads
    .iter()
    .find(|view| thread_key(view) == *key || resolved_key(view) == *key)
  else {
    return Element::new();
  };
  if view.state == ThreadState::Resolved && !env.state.opened.get().0.contains(key) {
    return wrap(resolved_fold(ctx, env, view));
  }
  wrap(card(ctx, env, view))
}

/// `inner` where a thread stands in the diff.
fn wrap(inner: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Loose.role())
        .left(ReviewSpacing::ThreadStart.role())
        .right(SpacingSize::Lg),
    )
    .background(ExtraColor::CodeBackground.role())
    .child(inner)
    .into()
}

/// A thread's box: its head and its messages.
fn card(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Element {
  let (_, _, stroke) = look::kind(view.thread.kind());
  let id = format!("review-thread-{}", view.thread.id());
  Column::new()
    .id(id)
    .describe("state", view.state.name())
    .describe("kind", view.thread.kind().as_str())
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Md)
    .child(head(ctx, env, view))
    .with_children(messages::messages(ctx, env, view))
    .with_children(replying(ctx, env, view))
    .into()
}

/// The box of a reply under an addressed comment, while it is open.
fn replying(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Option<Element> {
  let reply = InThread::FollowUp(view.thread.id());
  (env.state.in_thread.get().0 == Some(reply)).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Sm))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(in_thread_box(ctx, env, view, reply))
      .into()
  })
}

/// The head: the kind, the lines, the state's tag and the actions.
fn head(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Element {
  let id = format!("review-thread-{}", view.thread.id());
  let (glyph, word, color) = look::kind(view.thread.kind());
  let (tag_words, tag_look) = look::tag(ctx, view);
  Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .child(icon(glyph, IconSize::Md, color.clone()))
    .child(
      Text::new(&ctx.t(word))
        .id(format!("{id}-kind"))
        .variant(ExtraTypography::InspectorSection.role())
        .color(color)
        .nowrap(),
    )
    .child(
      Text::new(&lines(ctx, view.thread.anchor().span()))
        .id(format!("{id}-lines"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(tag(&format!("{id}-tag"), &tag_words, tag_look))
    .with_children(actions(ctx, env, view))
    .into()
}

/// What can be done with the thread now, at its head's end.
fn actions(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Vec<Element> {
  if env.read_only {
    return Vec::new();
  }
  let id = format!("review-thread-{}", view.thread.id());
  let thread = view.thread.id();
  match &view.state {
    ThreadState::NotSent | ThreadState::CouldNotSend(_) => unsent(ctx, env, view),
    ThreadState::Answered | ThreadState::AnswerStopped => vec![
      ask_again(ctx, env, view, (&format!("{id}-ask-again"), ButtonKind::Ghost)),
      resolve(ctx, env, (&id, thread), true),
    ],
    ThreadState::Addressed { .. } => {
      let state = env.state.clone();
      let reply = ActionButton::new(
        format!("{id}-reply"),
        ButtonKind::Ghost,
        ctx.t("review_page.thread.reply"),
      )
      .icon(Icon::Reply)
      .snug()
      .on_click(move || state.open_in_thread(InThread::FollowUp(thread), String::new()));
      vec![reply.into(), resolve(ctx, env, (&id, thread), true)]
    }
    ThreadState::Outdated { .. } => vec![resolve(ctx, env, (&id, thread), true)],
    ThreadState::Resolved => vec![resolve(ctx, env, (&id, thread), false)],
    _ => Vec::new(),
  }
}

/// *Edit* and *Delete* of the comment not sent yet.
fn unsent(ctx: &mut Ctx, env: &Env, view: &ThreadView) -> Vec<Element> {
  let id = format!("review-thread-{}", view.thread.id());
  let Some(comment) = view.thread.unsent().last() else {
    return Vec::new();
  };
  let (state, message, words) = (env.state.clone(), comment.id(), comment.body().to_owned());
  let edit = IconButton::new(format!("{id}-edit"), Icon::Pencil, ctx.t("review_page.thread.edit"))
    .small()
    .on_click(move || state.open_in_thread(InThread::Edit(message), words.clone()))
    .render(ctx);
  let writes = env.writes.clone();
  let delete = IconButton::new(format!("{id}-delete"), Icon::Trash2, ctx.t("review_page.thread.delete"))
    .small()
    .on_click(move || writes.run(Write::Keep(Kept::Delete(vec![message]))))
    .render(ctx);
  vec![edit, delete]
}

/// *Resolve* folds the thread; `false`: *Reopen* unfolds it.
fn resolve(ctx: &Ctx, env: &Env, (id, thread): (&str, orchester_core::id::ReviewThreadId), resolve: bool) -> Element {
  let (name, key, glyph) = if resolve {
    ("resolve", "review_page.thread.resolve", Icon::Check)
  } else {
    ("reopen", "review_page.thread.reopen", Icon::RotateCcw)
  };
  let writes = env.writes.clone();
  ActionButton::new(format!("{id}-{name}"), ButtonKind::Ghost, ctx.t(key))
    .icon(glyph)
    .snug()
    .on_click(move || writes.run(Write::Keep(Kept::Resolve(thread, resolve))))
    .into()
}

/// *Ask again*: the question's latest words go to Implement once more.
fn ask_again(ctx: &Ctx, env: &Env, view: &ThreadView, (id, kind): (&str, ButtonKind)) -> Element {
  let (writes, thread) = (env.writes.clone(), view.thread.id());
  let words = MessageBody::new(view.thread.latest_ask().body()).ok();
  let button = ActionButton::new(id.to_owned(), kind, ctx.t("review_page.thread.askAgain"))
    .icon(if kind == ButtonKind::Primary {
      Icon::RotateCcw
    } else {
      Icon::CornerDownRight
    })
    .snug();
  match words {
    Some(words) => button
      .on_click(move || writes.run(Write::AskAgain(thread, words.clone())))
      .into(),
    None => button.disabled().into(),
  }
}

/// A file's outdated threads folded into one line: *N outdated*, which opens them.
fn outdated_fold(ctx: &Ctx, env: &Env, path: &str) -> Element {
  let count = env
    .review
    .threads
    .iter()
    .filter(|view| view.thread.anchor().path() == path && matches!(view.state, ThreadState::Outdated { .. }))
    .count();
  let key = outdated_key(path);
  let open = env.state.opened.get().0.contains(&key);
  let words = ctx.t_args("review_page.thread.outdatedFold", [("count", count.to_string())]);
  fold_line(
    env,
    (format!("review-outdated-{path}"), key, open),
    (Icon::History, &words),
  )
}

/// A resolved thread folded into a line: its kind, lines and first words, which opens it.
fn resolved_fold(ctx: &Ctx, env: &Env, view: &ThreadView) -> Element {
  let (_, word, _) = look::kind(view.thread.kind());
  let first = view
    .thread
    .messages()
    .first()
    .map(|message| message.body().to_owned())
    .unwrap_or_default();
  let words = ctx.t_args(
    "review_page.thread.resolvedFold",
    [
      ("kind", ctx.t(word).to_string()),
      ("lines", lines(ctx, view.thread.anchor().span()).to_string()),
      ("words", first),
    ],
  );
  let key = resolved_key(view);
  fold_line(
    env,
    (format!("review-thread-{}", view.thread.id()), key, false),
    (Icon::Check, &words),
  )
}

/// One line on `surface_raised` that opens or closes the fold of `key`.
fn fold_line(env: &Env, (id, key, open): (String, Arc<str>, bool), (glyph, words): (Icon, &str)) -> Element {
  let state = env.state.clone();
  pressable(move || state.toggle_fold(key.clone()))
    .id(id)
    .describe("open", open.to_string())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Snug.role(),
    ))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(
      if open { Icon::ChevronDown } else { Icon::ChevronRight },
      IconSize::Md,
      PaletteColor::TextMuted,
    ))
    .child(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// Whether `view`'s question waits for an answer.
pub(super) fn answering(view: &ThreadView) -> bool {
  view.thread.kind() == ThreadKind::Question
    && matches!(view.state, ThreadState::Asked | ThreadState::Answering)
    && matches!(view.thread.latest_ask().said(), Said::Question(_))
}
