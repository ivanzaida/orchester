//! THREADS beside Files changed (`Threads head` `kVI2F`, `Side thread · …` rows in `ggxpO`'s side panel): how many
//! threads are open and drafted, that they stay in Orchester, and a row for each open thread and each new thread
//! being written: its state and where it is. A press opens the thread's file with the thread in view.

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
use orchester_core::id::FeedbackId;

use super::{Composing, Held, parts::visibility, thread::Env, words};
use crate::{
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, PullRequestsSpacing, PullRequestsText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// A side row's height (`Side thread · …`).
const ROW_HEIGHT: f32 = 30.0;

/// THREADS: the open threads and the new ones being written.
pub fn threads(ctx: &mut Ctx, env: &Env) -> Element {
  let open: Vec<_> = env.board.open().collect();
  let drafts = env.state.drafts.new_threads();
  let count = if drafts > 0 {
    ctx.t_args(
      "pull_requests.feedback.threads.openDrafts",
      [("count", open.len().to_string()), ("drafts", drafts.to_string())],
    )
  } else {
    ctx.t_args(
      "pull_requests.feedback.threads.open",
      [("count", open.len().to_string())],
    )
  };
  let head = Column::new()
    .id("feedback-threads-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(PullRequestsSpacing::FeedbackPanel.role())
        .bottom(ExtraSpacing::Snug.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("pull_requests.feedback.threads.heading"))
            .variant(ExtraTypography::ColumnHeading.role())
            .nowrap(),
        )
        .child(
          Text::new(&count)
            .id("feedback-threads-count")
            .variant(PullRequestsText::FileCount.role())
            .nowrap(),
        )
        .child(Row::new().flex(1.0))
        .child(visibility(ctx, "feedback-threads-visibility", true)),
    );
  let mut rows: Vec<Element> = Vec::new();
  if let Some(Composing::Lines { path, span, .. }) = env.state.composing.get() {
    let file = path.rsplit('/').next().unwrap_or(&path).to_owned();
    rows.push(draft_row(ctx, &file, span).into());
  }
  let Held(reveal) = env.state.reveal.get();
  for thread in open {
    rows.push(thread_row(ctx, env, thread, reveal == Some(thread.id)));
  }
  Column::new()
    .id("feedback-threads")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .child(head)
    .with_children(rows)
    .into()
}

/// A new thread being written, on its lines (`Side thread · sender.rs 54` in `JGBXx`).
fn draft_row(ctx: &Ctx, file: &str, span: orchester_core::pull_request::feedback::LineSpan) -> Row {
  side_row("feedback-threads-draft")
    .background(ExtraColor::Hover.role())
    .child(icon(Icon::PencilLine, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("pull_requests.feedback.state.draft"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&ctx.t_args(
        "pull_requests.feedback.anchor.short",
        [("file", file.to_owned()), ("lines", words::span_text(span))],
      ))
      .variant(ExtraTypography::MonoNote.role())
      .nowrap(),
    )
}

fn side_row(id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
}

/// An open thread: its state and where it is; a press shows it in its file.
fn thread_row(
  ctx: &mut Ctx,
  env: &Env,
  thread: &orchester_workspace::pull_requests::feedback::ThreadView,
  shown: bool,
) -> Element {
  let look = words::look(ctx, thread);
  let open = open(env, (thread.id, thread.path().map(Arc::from)));
  let row = pressable(move || open())
    .id(format!("feedback-threads-{}", thread.id))
    .describe("state", if shown { "selected" } else { "enabled" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(look.icon, IconSize::Base, look.color.clone()))
    .child(
      Text::new(&look.word)
        .variant(TypographyStyle::Label)
        .color(words::word_color(&look))
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&words::short_anchor(ctx, thread))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap(),
    );
  if shown {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row.into()
  }
}

/// Shows thread `id` in its file `path`, or beside the Agent feedback list when it is the whole pull request's.
fn open(env: &Env, (id, path): (FeedbackId, Option<Arc<str>>)) -> Arc<dyn Fn() + Send + Sync> {
  let navigation = env.navigation.clone();
  let (project, number, reveal) = (env.project.open.project.id(), env.number, env.state.reveal.clone());
  Arc::new(move || {
    reveal.set(Held(Some(id)));
    let page = match &path {
      Some(path) => ProjectPage::PullRequest(number, Some(path.to_string())),
      None => ProjectPage::PullRequestFeedback(number, Some(id)),
    };
    navigation.replace(Route::Project(project, page));
  })
}
