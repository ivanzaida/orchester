//! *Commits* in the review before merging (`Commits` `JVHZb` in `wN5Bz`): the commits the run's work is made of, newest
//! first, each a `Commit row` (`wEWkY`: its short hash in mono, its subject and when it was committed, 30 high over a
//! `border` line) in an outlined list, with their count beside the title. Read off the UI thread once per checkpoint
//! and whenever the queue changes, as a sync adds its merge commit.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::LoggedCommit;
use orchester_workspace::{
  background,
  display::{clock, short_revision},
  messages::Message,
  tasks::changes::{RunRange, run_commits_in},
};

use super::super::{TaskContext, ready::changed::note};
use crate::{
  services::Services,
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, TaskText},
  ui::{
    card::{CardSurface, card},
    message::message_text,
  },
};

/// A commit's row (`wEWkY`).
const ROW_HEIGHT: f32 = 30.0;

/// The run's commits as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Commits(Arc<Vec<LoggedCommit>>);

impl lurq::app::component::DevtoolsInspectable for Commits {}

/// The commits of `task`'s latest run, read again when its checkpoint or the queue (`queue`, its changes so far)
/// changes; `None` while they are read.
pub(super) fn read(ctx: &mut Ctx, task: &TaskContext, queue: u64) -> Option<Result<Commits, Message>> {
  let programs = Services::of(ctx).programs;
  let range = task.detail.run.as_ref().map(|run| RunRange::of(&run.run));
  let deps = (task.project.open.repo_root.clone(), range, queue);
  let load = ctx.future(deps, move |(root, range, _)| {
    let programs = programs.clone();
    async move {
      let Some(range) = range else {
        return Ok(Commits(Arc::default()));
      };
      background::run(move || run_commits_in(&root, &programs, &range))
        .await?
        .map(|commits| Commits(Arc::new(commits)))
        .map_err(|error| {
          tracing::warn!(%error, "cannot read the run's commits");
          error.message()
        })
    }
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(commits), None) => Some(Ok(commits)),
    (None, None) => None,
  }
}

/// *Commits* with their count and rows, or why they are not shown.
pub(super) fn section(ctx: &Ctx, read: Option<&Result<Commits, Message>>) -> Element {
  let (count, body) = match read {
    None => (None, note(&ctx.t("delivery.task.review.readingCommits"))),
    Some(Err(error)) => (None, note(&message_text(ctx, error))),
    Some(Ok(Commits(commits))) if commits.is_empty() => (Some(0), note(&ctx.t("delivery.task.review.noCommits"))),
    Some(Ok(Commits(commits))) => (
      Some(commits.len()),
      card("task-review-commits-list", CardSurface::Outline)
        .padding(Padding::symmetric(
          ExtraSpacing::Tight.role(),
          ExtraSpacing::Hair.role(),
        ))
        .with_children(commits.iter().map(row))
        .into(),
    ),
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("delivery.task.review.commits")).variant(TaskText::SectionTitle.role()))
    .with_children(count.map(|count| {
      Text::new(&count.to_string())
        .id("task-review-commits-count")
        .variant(DeliveryText::Count.role())
    }))
    .child(Spacer::new().flex(1.0));
  Column::new()
    .id("task-review-commits")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(body)
    .into()
}

fn row(commit: &LoggedCommit) -> Element {
  let hash = short_revision(commit.revision.as_str());
  Row::new()
    .id(format!("task-review-commit-{hash}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(hash).variant(DeliveryText::File.role()).nowrap())
    .child(
      Text::new(&commit.subject)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(&clock(commit.at))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}
