//! The toolbar over the files (`Changes toolbar` `lX6yt`, 44 high, 32 from the sides, a `border` line under
//! it): the run's base branch and revision, an arrow, its latest checkpoint with the step it followed and
//! its time and the *Checkpointed* badge, and at the end how large the checkpointed changes are. Before
//! the first checkpoint it says there is none yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{diff::Changes, revision::Revision};
use orchester_workspace::{
  display::{clock, short_revision},
  tasks::{changes::Checkpointed, detail::RunView},
};

use super::grouped;
use crate::{
  theme::IconSize,
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// Height of the toolbar (`lX6yt`).
const TOOLBAR_HEIGHT: f32 = 44.0;

pub fn toolbar(ctx: &Ctx, run: &RunView, checkpointed: &Checkpointed) -> Element {
  let base = short_revision(run.run.base().as_str());
  let base = match run.run.base_branch() {
    Some(branch) => ctx.t_args(
      "changes.toolbar.base",
      [("branch", branch.as_str()), ("revision", base)],
    ),
    None => base.into(),
  };
  let row = Row::new()
    .id("changes-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Section))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::GitCompare, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&base)
        .id("changes-base")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextMuted));
  let (head, size) = match checkpointed {
    Checkpointed::NoCheckpoint => return row.child(detail(&ctx.t("changes.toolbar.noCheckpoint"))).into(),
    Checkpointed::Read { head, changes } => {
      let diff = &changes.diff;
      let files = u32::try_from(diff.files().len()).expect("a diff Orchester reads has at most MAX_DIFF_FILES files");
      (
        head,
        Changes {
          files,
          additions: diff.additions(),
          deletions: diff.deletions(),
        },
      )
    }
    Checkpointed::TooLarge { head, size } => (head, *size),
  };
  row
    .child(
      Text::new(short_revision(head.as_str()))
        .id("changes-head")
        .variant(TypographyStyle::Mono)
        .nowrap(),
    )
    .child(detail(&checkpoint(ctx, run, head)))
    .child(badge(
      "changes-checkpointed",
      BadgeTone::Success,
      &ctx.t("changes.toolbar.checkpointed"),
    ))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&size_text(ctx, size))
        .id("changes-size")
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

fn detail(text: &str) -> Text {
  Text::new(text)
    .id("changes-checkpoint")
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
    .nowrap()
}

/// "checkpoint after 03 Implement · 16:36", while `head` is still the run's latest checkpoint; a newer
/// one is being read otherwise.
fn checkpoint(ctx: &Ctx, run: &RunView, head: &Revision) -> std::sync::Arc<str> {
  let latest = run
    .run
    .last_checkpoint()
    .filter(|checkpoint| checkpoint.revision() == head);
  let Some(checkpoint) = latest else {
    return ctx.t("changes.toolbar.earlierCheckpoint");
  };
  let time = clock(checkpoint.created_at());
  match run.step_of(checkpoint.execution()) {
    Some(step) => ctx.t_args(
      "changes.toolbar.checkpointAfter",
      [
        ("number", format!("{:02}", step.number)),
        ("step", step.name.to_string()),
        ("time", time),
      ],
    ),
    None => ctx.t_args("changes.toolbar.checkpointAt", [("time", time)]),
  }
}

/// "4 files · +102 / −14".
fn size_text(ctx: &Ctx, size: Changes) -> std::sync::Arc<str> {
  let key = if size.files == 1 {
    "changes.toolbar.size.one"
  } else {
    "changes.toolbar.size.other"
  };
  ctx.t_args(
    key,
    [
      ("files", grouped(ctx, size.files)),
      ("additions", grouped(ctx, size.additions)),
      ("deletions", grouped(ctx, size.deletions)),
    ],
  )
}
