//! The review page while its run's changes are read (`Review - v2` 10 `Z655x`, rule 9 of `b6M3lS`): the crumbs, the
//! title with its buttons and the status line show at once from the row the Review list read (the list last read stays
//! in the project's feed), and only the files and the diff are bones, in the geometry of the parts that replace them,
//! so nothing moves when the read lands. Opened directly, with no such row, the title and the status are bones too.
//! The status says what the row knows: where the run stands, its run, its steps and its size; its head and its base
//! come at its end once read. *Approve and merge* waits, as it cannot act on a head not read yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::project::TaskKey;
use orchester_workspace::tasks::review_list::{ReviewRow, ReviewState};

use super::{
  files::{FILES_WIDTH, ROW_HEIGHT},
  frame,
  head::{Standing, approve_button, open_task, size_words, status_line},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::merges::{
    attempt::head::{title, title_row},
    words::joined,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::skeleton::{FromFirstFrame, Skeleton, bone, fading, line, round_bone},
};

/// The files' bones, one for each row they stand for; the lower ones fade.
const FILE_BONES: [f32; 8] = [60.0, 120.0, 100.0, 140.0, 90.0, 110.0, 130.0, 80.0];
/// The diff's bones: a line of code's width each (`pGoJ2`).
const DIFF_BONES: [f32; 12] = [
  260.0, 420.0, 380.0, 460.0, 300.0, 400.0, 350.0, 440.0, 280.0, 390.0, 330.0, 250.0,
];
const BONE_HEIGHT: f32 = 10.0;
/// The words' bones of the title and the status line, as wide as the design draws them (`U80Tg`, `mkouq`, `RJeoP`).
const TITLE_BONE: f32 = 228.0;
const STANDING_BONE: f32 = 141.0;
const META_BONE: f32 = 386.0;

/// The page while its run's changes are read.
pub(super) fn page(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Element {
  ctx.provide(FromFirstFrame);
  let row = known(project, key);
  let navigation = Navigation::of(ctx);
  let header = header(ctx, (project, &navigation), key, row.as_ref());
  let status = status(ctx, row.as_ref());
  let columns = Skeleton::new("review-reading", ctx.t("review_page.reading"))
    .fills()
    .show(ctx, columns(ctx));
  frame(ctx, project, (key, row.as_ref().map(|row| row.number)))
    .child(header)
    .child(status)
    .child(columns)
    .into()
}

/// The row the Review list read for the task `key`, when it did.
fn known(project: &ShownProject, key: &TaskKey) -> Option<ReviewRow> {
  project.feed.review.with_untracked(|list| {
    let list = list.as_ref()?;
    list
      .waiting
      .iter()
      .chain(&list.with_agent)
      .chain(&list.approved_today)
      .find(|row| row.key == *key)
      .cloned()
  })
}

/// The title, or a bone where it comes, with *Open task* and *Approve and merge*, which waits for the head.
fn header(ctx: &mut Ctx, shown: (&ShownProject, &Navigation), key: &TaskKey, row: Option<&ReviewRow>) -> Element {
  let buttons = buttons(ctx, shown, key);
  match row {
    Some(row) => title(&row.title, buttons),
    None => {
      let bone = Skeleton::new("review-title-reading", ctx.t("review_page.reading"))
        .inline()
        .show(ctx, line(ctx, TypographyStyle::Heading, TITLE_BONE));
      title_row(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(bone), buttons)
    }
  }
}

fn buttons(ctx: &Ctx, (project, navigation): (&ShownProject, &Navigation), key: &TaskKey) -> Vec<Element> {
  vec![
    open_task(ctx, navigation, project, key).into(),
    approve_button(ctx).disabled().into(),
  ]
}

/// Where the run stands, as far as its row says: a run that was approved is not one this page reviews.
fn standing(state: &ReviewState) -> Option<Standing> {
  match state {
    ReviewState::Ready | ReviewState::CommentsNotSent { .. } => Some(Standing::Waiting),
    ReviewState::ChangedSinceReview { .. } => Some(Standing::Changed),
    ReviewState::ImplementOn { .. } => Some(Standing::AtWork),
    ReviewState::Conflicts { .. } | ReviewState::Merged { .. } | ReviewState::Merging { .. } => None,
  }
}

/// The status line from the row, or its bones.
fn status(ctx: &mut Ctx, row: Option<&ReviewRow>) -> Element {
  let Some((row, standing)) = row.and_then(|row| standing(&row.state).map(|standing| (row, standing))) else {
    let bones = Row::new()
      .id("review-status")
      .describe("status", "reading")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(round_bone(IconSize::Md.size()))
      .child(line(ctx, ExtraTypography::Strong.role(), STANDING_BONE))
      .child(line(ctx, TypographyStyle::Description, META_BONE));
    return Skeleton::new("review-status-reading", ctx.t("review_page.reading")).show(ctx, bones);
  };
  let mut parts = vec![ctx.t_args("review_page.status.run", [("run", row.number.get().to_string())])];
  if standing != Standing::AtWork {
    parts.push(ctx.t("review_page.status.stepsPassed"));
  }
  if let Some(size) = row.size {
    let files = usize::try_from(size.files).unwrap_or(usize::MAX);
    parts.push(size_words(ctx, (files, size.additions, size.deletions)));
  }
  status_line(ctx, standing, &joined(ctx, &parts))
}

/// The files and the diff as bones, where the files and the diff will be.
fn columns(ctx: &Ctx) -> Row {
  Row::new()
    .id("review-reading-columns")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tabs.role())
    .child(files(ctx))
    .child(diff())
}

/// The list's head and a row of bones for each of its first files.
fn files(ctx: &Ctx) -> Column {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(line(ctx, ExtraTypography::FormSection.role(), FILE_BONES[0]))
    .child(Row::new().flex(1.0))
    .child(line(ctx, TypographyStyle::Description, FILE_BONES[7]));
  let rows = FILE_BONES.iter().map(|width| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .align_items(Alignment::Center)
      .child(bone(*width, BONE_HEIGHT))
  });
  Column::new()
    .id("review-files-reading")
    .width(Dimension::Px(FILES_WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(head)
    .with_children(fading(rows.collect()))
}

/// The diff's panel, in the frame the diff has, with lines of code as bones.
fn diff() -> Column {
  let lines = DIFF_BONES.iter().map(|width| bone(*width, BONE_HEIGHT));
  Column::new()
    .id("review-diff-reading")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .padding(Padding::new().top(ExtraSpacing::Section.role()).left(SpacingSize::Md))
    .spacing(ExtraSpacing::Section.role())
    .with_children(fading(lines.collect()))
}
