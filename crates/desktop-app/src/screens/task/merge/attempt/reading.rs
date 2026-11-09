//! The attempt page while its run's merge attempts are read for the first time (`Review - v2` 13i `yHy6H`, rule 26 of
//! `b6M3lS`): the crumbs, the title and *Copy details* show at once, and what is read (the status, *What stopped it*,
//! the facts, *Checks* and *Attempts*) is bones, each the size of what replaces it, with the eight stages named, so
//! nothing moves when the read lands. The bar comes with the read. A page opened again shows the attempts last read
//! instead (the project's feed keeps them) while it reads them anew, so this stands only for a run read for the first
//! time.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{integration::attempt::AttemptStage, project::TaskKey, run::RunNumber};

use super::{
  checks::{NUMBER_WIDTH, framed, line as check_line},
  facts::log_folder,
  frame,
  head::{heading, wrapping_title},
  section_head_with,
  words::{stage_name, stage_where},
};
use crate::{
  project::ShownProject,
  theme::{AttemptText, DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    skeleton::{FromFirstFrame, Skeleton, bone, line, round_bone},
  },
};

/// The stages in the order an attempt lists them (`Check 01` … `Check 08`): the preflight takes the project database
/// before the integration lock, and only an automation's attempt passes its policy first.
const STAGES: [AttemptStage; 8] = [
  AttemptStage::ProjectDatabase,
  AttemptStage::IntegrationLock,
  AttemptStage::ScratchFolder,
  AttemptStage::CheckoutIdentity,
  AttemptStage::GitMetadata,
  AttemptStage::ExistingResult,
  AttemptStage::Merge,
  AttemptStage::Record,
];
/// The bones' widths, as the design draws them (`LQSR4`, `qTPT5`, `pGKLt`, `wqXcS`, the facts' and the checks').
const BONE_HEIGHT: f32 = 10.0;
const STATUS_WORD: f32 = 141.0;
const STATUS_META: f32 = 380.0;
const STRIP_TITLE: f32 = 100.0;
const STRIP_LINES: [f32; 2] = [960.0, 620.0];
const FACTS: [(&str, f32); 5] = [
  ("merges.page.properties.branch", 90.0),
  ("merges.page.properties.reviewedAt", 56.0),
  ("merges.page.properties.into", 124.0),
  ("merges.page.properties.checkout", 130.0),
  ("merges.page.properties.folder", 140.0),
];
const HEADING_COUNT: f32 = 110.0;
const RESULT: f32 = 80.0;
const WHERE: f32 = 160.0;
const ATTEMPT_COUNT: f32 = 12.0;
const ATTEMPT_PARTS: [f32; 3] = [40.0, 60.0, 60.0];
const ATTEMPT_OUTCOME: f32 = 260.0;
/// The strip's title and lines (`oHrX9`, `XLl7f`).
const STRIP_TITLE_HEIGHT: f32 = 15.0;
const STRIP_LINE_HEIGHT: f32 = 17.0;
const STRIP_ICON: f32 = 14.0;
const TITLE_BONE: f32 = 228.0;

/// The page while the attempts of run `number` of the task `key` are read.
pub(super) fn page(ctx: &mut Ctx, project: &ShownProject, (key, number): (&TaskKey, RunNumber)) -> Element {
  ctx.provide(FromFirstFrame);
  let title = title(ctx, project, key);
  let copy = ActionButton::new(
    "merge-attempt-copy-details",
    ButtonKind::Ghost,
    ctx.t("merges.page.copyDetails"),
  )
  .icon(Icon::Copy)
  .disabled();
  let body = Skeleton::new("merge-attempt-reading", ctx.t("merges.page.reading"))
    .fills()
    .show(ctx, body(ctx, project));
  frame(ctx, project, (key, number))
    .child(wrapping_title(title, vec![copy.into()]))
    .child(body)
    .into()
}

/// The task's title when the project's tasks are read, else a bone.
fn title(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Element {
  let known = project.feed.tasks.with(|tasks| {
    let tasks = tasks.ready()?;
    tasks
      .tasks
      .iter()
      .chain(tasks.not_shared.iter())
      .find(|task| task.key() == key)
      .map(|task| task.details().title.as_str().to_owned())
  });
  match known {
    Some(title) => heading(&title),
    None => Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(
        Skeleton::new("merge-attempt-title-reading", ctx.t("merges.page.reading"))
          .inline()
          .show(ctx, line(ctx, AttemptText::Title.role(), TITLE_BONE)),
      )
      .into(),
  }
}

/// The status line, the strip, the facts and the sections, as bones.
fn body(ctx: &Ctx, project: &ShownProject) -> Column {
  Column::new()
    .id("merge-attempt-reading-bones")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .child(status(ctx))
    .child(strip())
    .child(facts(ctx, project))
    .child(sections(ctx))
}

fn status(ctx: &Ctx) -> Row {
  Row::new()
    .id("merge-attempt-status")
    .describe("standing", "reading")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(round_bone(IconSize::Md.size()))
    .child(line(ctx, ExtraTypography::Strong.role(), STATUS_WORD))
    .child(bone(STATUS_META, BONE_HEIGHT))
}

/// *What stopped it*'s strip: a round bone for the glyph, a title and two lines.
fn strip() -> Row {
  let lines = STRIP_LINES.iter().map(|width| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STRIP_LINE_HEIGHT))
      .align_items(Alignment::Center)
      .child(shrinking(*width))
  });
  Row::new()
    .id("merge-attempt-what-reading")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .height(Dimension::Px(18.0))
        .align_items(Alignment::Center)
        .child(round_bone(STRIP_ICON)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .height(Dimension::Px(STRIP_TITLE_HEIGHT))
            .align_items(Alignment::Center)
            .child(bone(STRIP_TITLE, BONE_HEIGHT)),
        )
        .with_children(lines),
    )
}

/// A bone that gives way when the room is short.
fn shrinking(width: f32) -> Column {
  bone(width, BONE_HEIGHT).flex_shrink(1.0).min_width(Dimension::Px(0.0))
}

/// The facts' keys with a bone for each value, and *Runner log folder*, which needs nothing read.
fn facts(ctx: &Ctx, project: &ShownProject) -> Row {
  let facts = FACTS.iter().map(|(key, width)| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t(key))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .child(bone(*width, BONE_HEIGHT))
  });
  Row::new()
    .id("merge-attempt-facts")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Tabs.role())
    .align_items(Alignment::Center)
    .with_children(facts)
    .child(Row::new().flex(1.0))
    .child(log_folder(ctx, &project.open.repo_root))
}

/// *Checks* with its eight stages named, and *Attempts* with one line.
fn sections(ctx: &Ctx) -> Column {
  let rows = STAGES.iter().enumerate().map(|(index, stage)| {
    let last = index + 1 == STAGES.len();
    let touches: Element = if matches!(stage, AttemptStage::ExistingResult | AttemptStage::Merge) {
      // What these two touch names the run's branches, which are not read yet.
      Row::new().flex(1.0).child(bone(WHERE, BONE_HEIGHT)).into()
    } else {
      Text::new(&stage_where(ctx, *stage, ("", "")))
        .variant(DeliveryText::Count.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .into()
    };
    framed(format!("merge-attempt-reading-check-{}", index + 1), last).child(
      check_line(false)
        .child(
          Row::new()
            .width(Dimension::Px(NUMBER_WIDTH))
            .child(Text::new(&format!("{:02}", index + 1)).variant(DeliveryText::Count.role())),
        )
        .child(round_bone(IconSize::Md.size()))
        .child(
          Text::new(&stage_name(ctx, *stage))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        )
        .child(touches)
        .child(bone(RESULT, BONE_HEIGHT)),
    )
  });
  let checks = Column::new()
    .id("merge-attempt-reading-checks")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(rows);
  let attempt = check_line(false)
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .child(
      Row::new()
        .width(Dimension::Px(72.0))
        .child(bone(ATTEMPT_PARTS[0], BONE_HEIGHT)),
    )
    .child(bone(ATTEMPT_PARTS[1], BONE_HEIGHT))
    .child(bone(ATTEMPT_PARTS[2], BONE_HEIGHT))
    .child(Row::new().flex(1.0))
    .child(shrinking(ATTEMPT_OUTCOME));
  let attempts = Column::new()
    .id("merge-attempt-reading-attempts")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(attempt);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(SpacingSize::Lg)
    .child(section(ctx, ("checks", "merges.page.checks"), HEADING_COUNT, checks))
    .child(section(
      ctx,
      ("attempts", "merges.page.attempts"),
      ATTEMPT_COUNT,
      attempts,
    ))
}

/// A section's heading, which is real, with a bone for its count, over `body`.
fn section(ctx: &Ctx, (key, title): (&str, &str), count: f32, body: Column) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section_head_with(
      &format!("merge-attempt-reading-{key}-head"),
      &ctx.t(title),
      (Some(bone(count, BONE_HEIGHT).into()), None),
    ))
    .child(body)
}
