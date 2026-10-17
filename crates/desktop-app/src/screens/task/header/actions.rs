//! The title row (`Title row` `ZjHxm` in `lUXff`): the task's title, and at its right Edit, *Remove
//! worktree* of the latest run once it ended, the run controls and Run again. A locked action says why
//! in full in its tooltip; under the actions one short line says why for all of them, except on a run
//! that cannot be read, whose Summary says it already.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  HeaderProps,
  controls::{Control, Reason},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::task::{PageState, RemovalPlace},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    tooltip::{Tooltip, Truncated},
  },
};

/// The row with `controls` after Edit and *Remove worktree*.
pub fn title_row(ctx: &mut Ctx, props: &HeaderProps, controls: Vec<Control>) -> Element {
  let mut reasons: Vec<Reason> = Vec::new();
  let mut actions: Vec<Element> = edit_button(ctx, props)
    .into_iter()
    .chain(remove_button(ctx, props))
    .map(Element::from)
    .collect();
  for control in controls {
    let Some(reason) = control.reason else {
      actions.push(control.button.into());
      continue;
    };
    let id = format!("task-action-tooltip-{}", actions.len());
    actions.push(Tooltip::new(id, reason.full.clone()).wrap(ctx, control.button));
    if !reasons.contains(&reason) {
      reasons.push(reason);
    }
  }
  // A run that cannot be read says why on its Summary; the line would say it twice.
  let line = (!reasons.is_empty() && props.task.detail.latest_unreadable.is_none()).then(|| {
    let shorts: Vec<&str> = reasons.iter().map(|reason| &*reason.short).collect();
    Text::new(&shorts.join(&ctx.t("task.reasons.join")))
      .id("task-action-reasons")
      .variant(TypographyStyle::Caption)
      .nowrap()
      .describe("role", "unavailable-reason")
  });
  let title = Truncated::new(
    "task-title",
    props.task.detail.task.details().title.as_str().to_owned(),
    TypographyStyle::Heading,
  )
  .render(ctx);
  Row::new()
    .id("task-title-row")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(title))
    .child(
      Column::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::End)
        .child(
          Row::new()
            .id("task-actions")
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .with_children(actions),
        )
        .with_children(line),
    )
    .into()
}

/// Edit, while the task can be edited.
fn edit_button(ctx: &mut Ctx, props: &HeaderProps) -> Option<ActionButton> {
  if !props.task.detail.editable() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let route = Route::Project(
    props.task.project.open.project.id(),
    ProjectPage::EditTask(props.task.detail.task.key().clone()),
  );
  Some(
    ActionButton::new("task-edit", ButtonKind::Ghost, ctx.t("task.edit"))
      .icon(Icon::Pencil)
      .opens(navigation.to(route)),
  )
}

/// *Remove worktree* of the latest run, once it has no live work and still has worktrees: it opens the
/// confirmation under the header. Older runs have theirs in their rows.
fn remove_button(ctx: &mut Ctx, props: &HeaderProps) -> Option<ActionButton> {
  let detail = &props.task.detail;
  let run = detail.run.as_ref()?;
  let record = detail.history.iter().find(|record| record.id == run.id())?;
  record.without_live_work()?;
  let left = props.task.left.of(run.id())?.ok()?;
  if left.worktrees.is_empty() {
    return None;
  }
  let button =
    ActionButton::new("task-remove-worktree", ButtonKind::Ghost, ctx.t("task.removeWorktree")).icon(Icon::FolderX);
  if props.task.jobs.run(run.id()).removing() {
    return Some(button.loading());
  }
  let state = PageState::of(ctx);
  let busy = detail
    .history
    .iter()
    .any(|record| props.task.jobs.run(record.id).busy());
  // Busy with another run, or its confirmation is open already.
  if busy || state.removal.get().run == Some(run.id()) {
    return Some(button.disabled());
  }
  let job = props.task.project.feed.jobs.of(run.id());
  let id = run.id();
  Some(button.on_click(move || {
    job.clear_failed_removal();
    state.open_removal(id, RemovalPlace::Header);
  }))
}
