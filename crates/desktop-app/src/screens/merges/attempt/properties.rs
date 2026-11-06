//! The attempt page's right column (`Properties` `AukEz` in 09b): Run, Branch, Reviewed at, Into, Checkout and Folder,
//! each a 12 `text_muted` key in an 84 column and its value in 12 `text_secondary` (an identifier in Geist Mono), 7
//! over and under; then *Open KEY*, which opens the run's task, and *Runner log folder*, which opens the project's
//! `.orchester/logs` in the system's file manager. 24 in from its `border` line at its start, 260 wide.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::short_revision,
  merge_queue::{BaseState, CheckoutState},
  tasks::merge_attempts::page::AttemptPage,
};

use super::actions::open_folder;
use crate::{
  navigation::link::Open,
  theme::{AttemptSpacing, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressed},
  },
};

/// The column's width (`AukEz`).
const WIDTH: f32 = 260.0;
/// A property's key column (`KAbWj`).
const KEY_WIDTH: f32 = 84.0;

/// How a property's value is set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Value {
  Words,
  Identifier,
}

/// The column of `page`, the project at `root`'s; *Open KEY* opens `task`.
pub(super) fn properties(ctx: &mut Ctx, page: &AttemptPage, root: &Path, task: Open) -> Element {
  let none = ctx.t("merges.page.properties.none");
  let reviewed = page
    .reviewed
    .as_ref()
    .map_or_else(|| none.clone(), |revision| Arc::from(short_revision(revision.as_str())));
  let folder = root.display().to_string();
  let logs = orchester_ipc::runner_log_dir(root);
  let rows = [
    ("run", "merges.page.properties.run", run(ctx, page), Value::Words),
    (
      "branch",
      "merges.page.properties.branch",
      Arc::from(page.branch.as_str()),
      Value::Identifier,
    ),
    (
      "reviewed",
      "merges.page.properties.reviewedAt",
      reviewed,
      Value::Identifier,
    ),
    (
      "into",
      "merges.page.properties.into",
      into(ctx, page, none),
      Value::Identifier,
    ),
    (
      "checkout",
      "merges.page.properties.checkout",
      checkout(ctx, page.checkout.as_ref()),
      Value::Words,
    ),
    (
      "folder",
      "merges.page.properties.folder",
      Arc::from(folder.as_str()),
      Value::Identifier,
    ),
  ]
  .into_iter()
  .map(|(id, key, value, kind)| property(&format!("merge-attempt-property-{id}"), &ctx.t(key), &value, kind));
  let open_key = ctx.t_args("merges.page.properties.openKey", [("key", page.run.key.to_string())]);
  Column::new()
    .id("merge-attempt-properties")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Xl))
    .spacing(ExtraSpacing::Tight.role())
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(rows)
    .child(Column::new().padding(Padding::new().top(SpacingSize::Sm)))
    .child(link(
      "merge-attempt-open-key",
      (Icon::FileText, &open_key),
      Press::Open(task),
    ))
    .child(link(
      "merge-attempt-log-folder",
      (Icon::FolderOpen, &ctx.t("merges.page.properties.logFolder")),
      Press::run(move || open_folder("open-runner-logs", &logs)),
    ))
    .into()
}

/// *#8 · Feature delivery v3*: the run's number, and the pipeline and version it pinned.
fn run(ctx: &Ctx, page: &AttemptPage) -> Arc<str> {
  let number = page.run.number.get().to_string();
  match &page.pipeline {
    (Some(name), version) => ctx.t_args(
      "merges.page.properties.runValue",
      [
        ("run", number),
        ("pipeline", name.clone()),
        ("version", version.to_string()),
      ],
    ),
    (None, _) => ctx.t_args("merges.page.properties.runNoPipeline", [("run", number)]),
  }
}

/// *master @ 73b2c9c*: the branch the run merges into and where it is now; `none` without one.
fn into(ctx: &Ctx, page: &AttemptPage, none: Arc<str>) -> Arc<str> {
  match (&page.run.base, &page.into_head) {
    (Some(base), Some(head)) => ctx.t_args(
      "merges.page.properties.intoValue",
      [("branch", base.as_str()), ("revision", short_revision(head.as_str()))],
    ),
    (Some(base), None) => Arc::from(base.as_str()),
    (None, _) => none,
  }
}

/// How the checkout stands: the branch it has, and whether it is clean.
fn checkout(ctx: &Ctx, base: Option<&BaseState>) -> Arc<str> {
  let Some(base) = base else {
    return ctx.t_args(
      "merges.page.properties.onBranch",
      [("branch", ctx.t("merges.page.properties.detached").to_string())],
    );
  };
  let branch = base.branch.as_str();
  match &base.checkout {
    CheckoutState::Clean => ctx.t_args("merges.page.properties.clean", [("branch", branch)]),
    CheckoutState::Uncommitted(files) if files.len() == 1 => {
      ctx.t_args("merges.page.properties.changedOne", [("branch", branch)])
    }
    CheckoutState::Uncommitted(files) => ctx.t_args(
      "merges.page.properties.changedOther",
      [("branch", branch.to_owned()), ("count", files.len().to_string())],
    ),
    CheckoutState::InProgress => ctx.t_args("merges.page.properties.inProgress", [("branch", branch)]),
    CheckoutState::OtherBranch(_) | CheckoutState::Beside(_) | CheckoutState::Unknown => {
      ctx.t_args("merges.page.properties.onBranch", [("branch", branch)])
    }
  }
}

/// A property's row: its key, and its value, which wraps in the column rather than lose its end: a branch or a folder
/// is read whole.
fn property(id: &str, key: &str, value: &str, kind: Value) -> Element {
  let style = match kind {
    Value::Words => ExtraTypography::Property.role(),
    Value::Identifier => ExtraTypography::PropertyMono.role(),
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(AttemptSpacing::CompactY.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(
      Text::new(key)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(KEY_WIDTH))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(style)
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// A link under the properties: its glyph and words, and an arrow at its end.
fn link(id: &str, (glyph, words): (Icon, &str), press: Press) -> Element {
  pressed(press)
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(AttemptSpacing::CompactY.role()))
    .spacing(SpacingSize::Sm)
    .describe("a11y_name", words.to_owned())
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}
