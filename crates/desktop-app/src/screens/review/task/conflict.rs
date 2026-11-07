//! A merge that conflicts on the review page (`Review - v2` 11 `CT6Gc`): under the conflicting lines a *Conflict* block
//! (`Conflict · lines 41–44` `rpvbP`) shows what the base branch has there now, and in the tray's place the conflict
//! bar (`Conflict bar` `DDrv4`, 52 high on `surface_raised` in a `border` stroke, radius 6, 16 at its sides): the
//! glyph, *2 files conflict with master* with what the agent does about it, *Try merge again*, which merges once more
//! when the base moved, and *Ask the agent to resolve*, which sends the run back to Implement with the base merged in.

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
use orchester_workspace::tasks::review_threads::{
  ReviewRefusal,
  conflicts::{ConflictRegion, ReviewConflicts},
};

use super::{Env, actions::Write, words::counted};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The bar's height (`DDrv4`) and a conflict block's head (`DQ7Fc`).
const BAR_HEIGHT: f32 = 52.0;
const HEAD_HEIGHT: f32 = 32.0;
/// The characters a base line's number and the room after it take before its code (`c6XRc`).
const NUMBER_COLUMN: usize = 8;

/// The key of the block of `path`'s `at`-th conflict.
pub(super) fn key(path: &str, at: usize) -> Arc<str> {
  format!("conflict:{path}:{at}").into()
}

/// The block under `key`, which [`key`] made; `None` when the page holds no such conflict.
pub(super) fn block(ctx: &Ctx, env: &Env, key: &str) -> Option<Element> {
  let (path, at) = key.strip_prefix("conflict:")?.rsplit_once(':')?;
  let region = env.page.conflicts.as_ref()?.regions_in(path).nth(at.parse().ok()?)?;
  Some(region_box(ctx, (path, at), region))
}

/// What the base branch has in the lines the run changed, under *Conflict · lines 41–44*.
fn region_box(ctx: &Ctx, (path, at): (&str, &str), region: &ConflictRegion) -> Element {
  let id = format!("review-conflict-{path}-{at}");
  let (first, last) = region.lines;
  let lines = if first == last {
    ctx.t_args("review_page.thread.line", [("line", first.to_string())])
  } else {
    ctx.t_args(
      "review_page.thread.lines",
      [("start", first.to_string()), ("end", last.to_string())],
    )
  };
  let head = Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::GitCompareArrows, IconSize::Md, PaletteColor::Danger))
    .child(
      Text::new(&ctx.t("review_page.conflict.title"))
        .variant(ExtraTypography::InspectorSection.role())
        .color(PaletteColor::Danger)
        .nowrap(),
    )
    .child(
      Text::new(&lines)
        .id(format!("{id}-lines"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  let base = Column::new()
    .id(format!("{id}-base"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Snug.role()))
    .background(ExtraColor::CodeBackground.role())
    .with_children(base_lines(ctx, region));
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
    .rounded(RadiusSize::Md)
    .clip()
    .child(head)
    .child(base)
    .into()
}

/// The base's lines, each after its number; one sentence when the base removed them.
fn base_lines(ctx: &Ctx, region: &ConflictRegion) -> Vec<Element> {
  if region.base_lines.is_empty() {
    return vec![
      Text::new(&ctx.t("review_page.conflict.removed"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .into(),
    ];
  }
  region
    .base_lines
    .iter()
    .map(|line| {
      Text::new(&format!("{:<NUMBER_COLUMN$}{}", line.number, line.text))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .into()
    })
    .collect()
}

/// The conflict bar; `None` while the run's merge conflicts nowhere.
pub(super) fn bar(ctx: &Ctx, env: &Env) -> Option<Element> {
  let conflicts = env.page.conflicts.as_ref()?;
  let base = base_name(ctx, conflicts);
  let title = counted_with_base(ctx, conflicts.files.len(), &base);
  let body = ctx.t_args("review_page.conflict.bar.body", [("base", &*base)]);
  Some(
    Row::new()
      .id("review-conflict-bar")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(BAR_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::horizontal(SpacingSize::Lg))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::GitCompareArrows, IconSize::Nav, PaletteColor::Danger))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&title)
              .id("review-conflict-title")
              .variant(ExtraTypography::Strong.role())
              .nowrap(),
          )
          .child(
            Text::new(&body)
              .id("review-conflict-body")
              .variant(TypographyStyle::Description)
              .color(PaletteColor::TextMuted)
              .nowrap(),
          ),
      )
      .child(try_again(ctx, env))
      .child(resolve(ctx, env))
      .into(),
  )
}

/// The base branch's name, or *its base branch* when the run recorded none.
pub(super) fn base_name(ctx: &Ctx, conflicts: &ReviewConflicts) -> Arc<str> {
  match &conflicts.base {
    Some(base) => Arc::from(base.as_str()),
    None => ctx.t("review_page.conflict.baseBranch"),
  }
}

/// *1 file conflicts with master* or *2 files conflict with master*.
fn counted_with_base(ctx: &Ctx, files: usize, base: &str) -> Arc<str> {
  let key = if files == 1 {
    "review_page.conflict.bar.titleOne"
  } else {
    "review_page.conflict.bar.titleOther"
  };
  ctx.t_args(key, [("count", files.to_string()), ("base", base.to_owned())])
}

/// *Try merge again*: merges the run at the head shown once more, which succeeds once the conflicts are gone.
fn try_again(ctx: &Ctx, env: &Env) -> Element {
  let button = ActionButton::new(
    "review-conflict-try-again",
    ButtonKind::Ghost,
    ctx.t("review_page.conflict.bar.tryAgain"),
  )
  .icon(Icon::RotateCcw);
  if env.writes.busy() {
    return button.disabled().into();
  }
  let (writes, head) = (env.writes.clone(), env.head.clone());
  button
    .on_click(move || writes.run(Write::Approve(head.clone(), Vec::new())))
    .into()
}

/// *Ask the agent to resolve*: sends the run back to Implement with the base merged in; unavailable without one.
fn resolve(ctx: &Ctx, env: &Env) -> Element {
  let button = ActionButton::new(
    "review-conflict-resolve",
    ButtonKind::Primary,
    ctx.t("review_page.conflict.bar.resolve"),
  )
  .icon(Icon::Bot);
  if env.page.implement.is_none() {
    return button
      .unavailable(message_text(ctx, &ReviewRefusal::NoImplementingStep.message()))
      .into();
  }
  if env.writes.busy() {
    return button.loading().into();
  }
  let writes = env.writes.clone();
  button.on_click(move || writes.run(Write::Resolve)).into()
}

/// How many conflicts `path` shows in its head: its regions, or one for a file that conflicts without lines to show.
pub(super) fn count_in(conflicts: &ReviewConflicts, path: &str) -> usize {
  if !conflicts.conflicts_in(path) {
    return 0;
  }
  conflicts.regions_in(path).count().max(1)
}

/// *2 conflicts* for the head of a file.
pub(super) fn tag_words(ctx: &Ctx, count: usize) -> Arc<str> {
  counted(
    ctx,
    ("review_page.diff.conflictsOne", "review_page.diff.conflictsOther"),
    count,
  )
}
