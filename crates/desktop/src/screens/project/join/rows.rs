//! The review of 10.21: the bar that ticks or unticks every task (`qrSY1`, `rWqoy`), the table of
//! local tasks (LOCAL KEY, TASK, WORKFLOW / RUN, BECOMES) and the note under it (`d5UVgm`).

use std::{num::NonZeroU32, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  project::TaskKey,
  run::{BranchName, TaskPhase},
};

use super::{JoinPage, Unticked};
use crate::{
  screens::project::{
    task_rows::status_copy,
    tasks::selection::{Marked, checkbox, marked_box},
  },
  sharing::project::{JoinReview, ReviewTask},
  tasks::organization::workflow_label,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
  },
};

/// Column widths of the review (`zm2d8`, `AqYpJ`, `rRC6h` in `UlHPB`): the tick, the local key,
/// workflow and run, and what the task becomes; the task takes the rest.
const TICK_WIDTH: f32 = 46.0;
const KEY_WIDTH: f32 = 88.0;
const WORKFLOW_WIDTH: f32 = 228.0;
const BECOMES_WIDTH: f32 = 162.0;
/// The review bar's height (`qrSY1` row).
const BAR_HEIGHT: f32 = 56.0;

impl JoinPage {
  /// *4 of 6 ticked* and what joining does with them; its box ticks or unticks every task that
  /// can be unticked.
  pub(super) fn review_bar(&self, ctx: &Ctx, review: &JoinReview) -> Element {
    let ticked = self.ticked(review);
    let total = review.tasks.len();
    let marked = match ticked.len() {
      0 => Marked::None,
      count if count == total => Marked::All,
      _ => Marked::Some,
    };
    let unticked = self.unticked.clone();
    let open: Vec<_> = review
      .tasks
      .iter()
      .filter(|row| !row.locked())
      .map(|row| row.task.id())
      .collect();
    let every = marked_box("join-tick-all", marked, move || {
      unticked.update(|Unticked(set)| {
        if set.is_empty() {
          set.extend(open.iter().copied());
        } else {
          set.clear();
        }
      });
    });
    Row::new()
      .id("join-review-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(BAR_HEIGHT))
      .padding_horizontal(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Notice.role())
      .child(every)
      .child(
        Text::new(&ctx.t_args(
          "sharing.join.ticked",
          [
            ("ticked", ticked.len().to_string().as_str()),
            ("total", total.to_string().as_str()),
          ],
        ))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
      )
      .child(
        Text::new(&summary(ctx, review, &ticked))
          .variant(TypographyStyle::Caption)
          .flex(1.0),
      )
      .into()
  }

  /// The column headings and a row per local task.
  pub(super) fn table(&self, ctx: &Ctx, review: &JoinReview) -> Element {
    let ticked = self.ticked(review);
    let mut next = review.next;
    let mut table = Column::new()
      .id("join-review")
      .width(Dimension::Pct(100.0))
      .child(headings(ctx));
    for row in &review.tasks {
      let becomes = ticked.contains(&row.task.id()).then(|| {
        let number = next;
        next = next.saturating_add(1);
        format!("{}-{number}", review.key.as_str())
      });
      table = table.child(self.row(ctx, (row, review.key.as_str()), becomes.as_deref()));
    }
    table.into()
  }

  fn row(&self, ctx: &Ctx, (row, project): (&ReviewTask, &str), becomes: Option<&str>) -> Element {
    let key = row.task.key().to_string();
    let tick: Element = if row.locked() {
      icon(Icon::SquareCheck, IconSize::Option, PaletteColor::TextMuted).into()
    } else {
      let (unticked, id) = (self.unticked.clone(), row.task.id());
      checkbox(format!("join-tick-{key}"), becomes.is_some(), move || {
        unticked.update(|Unticked(set)| {
          if !set.remove(&id) {
            set.insert(id);
          }
        });
      })
    };
    Row::new()
      .id(format!("join-row-{key}"))
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Md)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Column::new().width(Dimension::Px(TICK_WIDTH)).child(tick))
      .child(
        Text::new(&key)
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextSecondary)
          .width(Dimension::Px(KEY_WIDTH)),
      )
      .child(task_cell(ctx, row, project))
      .child(workflow_cell(ctx, row))
      .child(becomes_cell(ctx, becomes))
      .into()
  }
}

/// What joining does with the ticked tasks: their new keys, the ones that stay here and those
/// claimed for the person.
fn summary(ctx: &Ctx, review: &JoinReview, ticked: &[TaskId]) -> Arc<str> {
  let key = review.key.as_str();
  let first = format!("{key}-{}", review.next);
  let more = u32::try_from(ticked.len().saturating_sub(1)).unwrap_or(u32::MAX);
  let last = format!("{key}-{}", review.next.saturating_add(more));
  let kept = review.tasks.len() - ticked.len();
  let claimed = review.tasks.iter().filter(|row| row.locked()).count();
  let mut parts = vec![match ticked.len() {
    0 => ctx.t("sharing.join.summary.uploadedNone"),
    1 => ctx.t_args("sharing.join.summary.uploaded.one", [("key", first.as_str())]),
    count => ctx.t_args(
      "sharing.join.summary.uploaded.other",
      [
        ("count", count.to_string().as_str()),
        ("first", first.as_str()),
        ("last", last.as_str()),
      ],
    ),
  }];
  if kept > 0 {
    parts.push(plural(ctx, "sharing.join.summary.kept", kept));
  }
  if claimed > 0 {
    parts.push(plural(ctx, "sharing.join.summary.claimed", claimed));
  }
  Arc::from(parts.join(&format!(" {} ", ctx.t("common.separator"))))
}

/// `key.one` or `key.other` with `count`.
fn plural(ctx: &Ctx, key: &str, count: usize) -> Arc<str> {
  if count == 1 {
    ctx.t(&format!("{key}.one"))
  } else {
    ctx.t_args(&format!("{key}.other"), [("count", count.to_string().as_str())])
  }
}

fn headings(ctx: &Ctx) -> Element {
  let heading = |key: &str| Text::new(&ctx.t(key)).variant(ExtraTypography::ColumnHeading.role());
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Column::new().width(Dimension::Px(TICK_WIDTH)))
    .child(heading("sharing.join.columns.key").width(Dimension::Px(KEY_WIDTH)))
    .child(heading("sharing.join.columns.task").flex(1.0))
    .child(heading("sharing.join.columns.workflow").width(Dimension::Px(WORKFLOW_WIDTH)))
    .child(heading("sharing.join.columns.becomes").width(Dimension::Px(BECOMES_WIDTH)))
    .into()
}

/// The title, the similar server task as a hint, and why a task cannot be unticked.
fn task_cell(ctx: &Ctx, row: &ReviewTask, project: &str) -> Element {
  let mut cell = Column::new().flex(1.0).spacing(ExtraSpacing::Indicator.role()).child(
    Text::new(row.task.details().title.as_str())
      .variant(ExtraTypography::FileName.role())
      .color(PaletteColor::TextPrimary),
  );
  if let Some(similar) = &row.similar {
    let hint = ctx.t_args(
      "sharing.join.similar",
      [
        ("project", project),
        ("key", similar.key.as_str()),
        ("title", similar.title.as_str()),
        ("by", similar.created_by.as_str()),
      ],
    );
    cell = cell.child(hint_line(Icon::Copy, &hint));
  }
  if let (true, Some((number, phase))) = (row.locked(), row.run) {
    let number = number.get().to_string();
    let reason = if phase == TaskPhase::Paused {
      ctx.t_args("sharing.join.lockedPaused", [("number", number.as_str())])
    } else {
      ctx.t_args("sharing.join.locked", [("number", number.as_str())])
    };
    cell = cell.child(hint_line(Icon::Lock, &reason));
  }
  cell.into()
}

fn hint_line(glyph: Icon, text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(text).variant(TypographyStyle::Caption).flex(1.0))
    .into()
}

/// The task's workflow status and where its latest run stands.
fn workflow_cell(ctx: &Ctx, row: &ReviewTask) -> Element {
  let run = match row.run {
    None => ctx.t("sharing.join.run.none"),
    Some((number, phase)) => {
      let phase_word = ctx.t(status_copy(phase).1);
      let key = if phase.run_not_ended() {
        "sharing.join.run.active"
      } else {
        "sharing.join.run.ended"
      };
      ctx.t_args(
        key,
        [("phase", &*phase_word), ("number", number.get().to_string().as_str())],
      )
    }
  };
  Column::new()
    .width(Dimension::Px(WORKFLOW_WIDTH))
    .spacing(ExtraSpacing::Indicator.role())
    .child(Row::new().child(workflow_label(ctx, row.task.organization().status())))
    .child(Text::new(&run).variant(ExtraTypography::Footnote.role()))
    .into()
}

/// The key the task gets, or *Not shared*.
fn becomes_cell(ctx: &Ctx, becomes: Option<&str>) -> Element {
  let cell = Row::new()
    .width(Dimension::Px(BECOMES_WIDTH))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center);
  match becomes {
    Some(key) => cell
      .child(icon(Icon::ArrowRight, IconSize::Small, PaletteColor::TextSecondary))
      .child(
        Text::new(key)
          .variant(ExtraTypography::MonoValue.role())
          .color(PaletteColor::TextPrimary),
      ),
    None => cell
      .child(icon(Icon::Archive, IconSize::Small, PaletteColor::TextSecondary))
      .child(Text::new(&ctx.t("sharing.notShared.label")).variant(TypographyStyle::Caption)),
  }
  .into()
}

/// What goes with each task and what stays attached (`d5UVgm`), with the branch of the first
/// uploaded task that has runs as the example.
pub(super) fn note(ctx: &Ctx, review: &JoinReview, ticked: &[TaskId]) -> Element {
  let key = review.key.as_str();
  let example = ticked.iter().enumerate().find_map(|(index, id)| {
    let row = review.tasks.iter().find(|row| row.task.id() == *id)?;
    let branch = row.branch.as_ref()?;
    let number = review.next.checked_add(u32::try_from(index).ok()?)?;
    let new = TaskKey::new(review.key.clone(), NonZeroU32::new(number)?);
    Some((branch.clone(), BranchName::for_next_run(&new, row.runs), new))
  });
  let text = example.map_or_else(
    || ctx.t_args("sharing.join.note", [("key", key)]),
    |(branch, next, task)| {
      ctx.t_args(
        "sharing.join.noteBranch",
        [
          ("key", key),
          ("branch", branch.as_str()),
          ("task", task.to_string().as_str()),
          ("next", next.as_str()),
        ],
      )
    },
  );
  Row::new()
    .id("join-note")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Info, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::CellLabel.role()).flex(1.0))
    .into()
}

/// *Try again* and *Back to the review* under a failed join (10.26 D).
pub(super) fn failure_actions(
  ctx: &Ctx,
  retry: impl Fn() + Send + Sync + 'static,
  back: impl Fn() + Send + Sync + 'static,
) -> Element {
  Row::new()
    .padding_top(ExtraSpacing::Tight.role())
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new(ctx.t("sharing.join.tryAgain"))
        .icon(Icon::RefreshCw)
        .id("join-retry")
        .on_click(retry)
        .render(ctx),
    )
    .child(
      ActionButton::new(ctx.t("sharing.join.backToReview"))
        .id("join-back")
        .on_click(back)
        .render(ctx),
    )
    .into()
}
