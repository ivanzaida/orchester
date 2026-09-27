//! The toolbar above the selected file's diff, as the pull request screen's (`SrS47`): the two
//! revisions the diff compares, the checkpoint it ends at (its step and time), and whether the
//! file's changes are checkpointed.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  display,
  tasks::{
    changes::{Checkpointed, ReadChanges},
    detail::RunView,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{icon::Icon, status_label::status_label},
};

/// Toolbar height (`SrS47`).
const TOOLBAR_HEIGHT: f32 = 54.0;

/// `shown` is the diff of the selected file: the checkpointed one, or the worktree's.
pub(super) fn toolbar(
  ctx: &Ctx,
  run: &RunView,
  checkpointed: &Checkpointed,
  shown: &ReadChanges,
  worktree: bool,
) -> Element {
  let from = display::short_revision(shown.from.as_str());
  let (range, detail, label) = if worktree {
    let detail = match checkpointed {
      Checkpointed::NoCheckpoint => ctx.t("runs.changes.worktreeFromBase"),
      _ => ctx.t_args("runs.changes.worktreeFromCheckpoint", [("revision", from)]),
    };
    (
      ctx.t_args("runs.changes.worktreeRange", [("from", from)]),
      detail,
      status_label(Icon::CircleDashed, &ctx.t("runs.changes.notCheckpointed")),
    )
  } else {
    let head = match checkpointed {
      Checkpointed::Read { head, .. } | Checkpointed::TooLarge { head, .. } => display::short_revision(head.as_str()),
      Checkpointed::NoCheckpoint => "",
    };
    (
      ctx.t_args("runs.changes.range", [("from", from), ("to", head)]),
      checkpoint(ctx, run, head),
      status_label(Icon::CircleCheck, &ctx.t("runs.changes.checkpointed")),
    )
  };
  Row::new()
    .id("changes-toolbar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&range)
        .id("changes-range")
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(
      Column::new().flex(1.0).child(
        Text::new(&detail)
          .id("changes-checkpoint")
          .variant(TypographyStyle::Caption)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      ),
    )
    .child(Spacer::new())
    .child(label)
    .into()
}

/// "Run 3 from its base to its latest checkpoint · Implement · 12:31", while `head` (short) is
/// still the latest checkpoint; a newer one is being read.
fn checkpoint(ctx: &Ctx, run: &RunView, head: &str) -> std::sync::Arc<str> {
  let number = run.run.number().get().to_string();
  let latest = run
    .run
    .last_checkpoint()
    .filter(|checkpoint| display::short_revision(checkpoint.revision().as_str()) == head);
  let Some(checkpoint) = latest else {
    return ctx.t_args("runs.changes.toEarlierCheckpoint", [("run", number.as_str())]);
  };
  let step = run.step_of(checkpoint.execution()).map_or("", |step| &*step.name);
  ctx.t_args(
    "runs.changes.toCheckpoint",
    [
      ("run", number.as_str()),
      ("step", step),
      ("time", display::clock(checkpoint.created_at()).as_str()),
    ],
  )
}
