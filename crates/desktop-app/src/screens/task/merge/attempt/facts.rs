//! The attempt's facts under *What stopped it* (`Facts` in 13a `i0K3xB`): Branch, Reviewed at, Into, Checkout and
//! Folder, each a 12 `text_muted` key and its value in Geist Mono 12 `text_secondary`, 6 apart and 20 from the next;
//! at the end *Runner log folder*, which opens the project's `.orchester/logs` in the system's file manager. The
//! checkout's value is in `warning` while files are left changed in it (13h `anuAO`).

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::short_revision,
  merge_queue::{BaseState, CheckoutState},
  tasks::merge_attempts::page::AttemptPage,
};

use super::actions::open_folder;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressed},
  },
};

/// The facts of `page`, the project at `root`'s.
pub(super) fn facts(ctx: &Ctx, page: &AttemptPage, root: &Path) -> Element {
  let none = ctx.t("merges.page.properties.none");
  let reviewed = page
    .reviewed
    .as_ref()
    .map_or_else(|| none.clone(), |revision| Arc::from(short_revision(revision.as_str())));
  let folder = root.display().to_string();
  let (checkout, files_changed) = checkout(ctx, page.checkout.as_ref());
  let facts = [
    (
      "branch",
      "merges.page.properties.branch",
      Arc::from(page.branch.as_str()),
      false,
    ),
    ("reviewed", "merges.page.properties.reviewedAt", reviewed, false),
    ("into", "merges.page.properties.into", into(ctx, page, none), false),
    ("checkout", "merges.page.properties.checkout", checkout, files_changed),
    (
      "folder",
      "merges.page.properties.folder",
      Arc::from(folder.as_str()),
      false,
    ),
  ]
  .into_iter()
  .map(|(id, key, value, warning)| fact(&format!("merge-attempt-fact-{id}"), &ctx.t(key), &value, warning));
  Row::new()
    .id("merge-attempt-facts")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Tabs.role())
    .align_items(Alignment::Center)
    .with_children(facts)
    .child(Row::new().flex(1.0))
    .child(log_folder(ctx, root))
    .into()
}

/// *Runner log folder ↗*: opens the project's `.orchester/logs` in the system's file manager.
pub(super) fn log_folder(ctx: &Ctx, root: &Path) -> Element {
  let logs = orchester_ipc::runner_log_dir(root);
  link(
    "merge-attempt-log-folder",
    (Icon::ArrowUpRight, &ctx.t("merges.page.properties.logFolder")),
    Press::run(move || open_folder("open-runner-logs", &logs)),
  )
}

/// *master @ 73b2c9c*: the branch the run merges into and where it is now; `none` without one.
fn into(ctx: &Ctx, page: &AttemptPage, none: Arc<str>) -> Arc<str> {
  match (&page.run.base, &page.into_head) {
    (Some(base), Some(head)) if !page.is_merged() => ctx.t_args(
      "merges.page.properties.intoValue",
      [("branch", base.as_str()), ("revision", short_revision(head.as_str()))],
    ),
    (Some(base), _) => Arc::from(base.as_str()),
    (None, _) => none,
  }
}

/// How the checkout stands: the branch it has and whether it is clean, and whether files are changed in it.
fn checkout(ctx: &Ctx, base: Option<&BaseState>) -> (Arc<str>, bool) {
  let Some(base) = base else {
    return (
      ctx.t_args(
        "merges.page.properties.onBranch",
        [("branch", ctx.t("merges.page.properties.detached").to_string())],
      ),
      false,
    );
  };
  let branch = base.branch.as_str();
  match &base.checkout {
    CheckoutState::Clean => (ctx.t_args("merges.page.properties.clean", [("branch", branch)]), false),
    CheckoutState::Uncommitted(files) if files.len() == 1 => (
      ctx.t_args("merges.page.properties.changedOne", [("branch", branch)]),
      true,
    ),
    CheckoutState::Uncommitted(files) => (
      ctx.t_args(
        "merges.page.properties.changedOther",
        [("branch", branch.to_owned()), ("count", files.len().to_string())],
      ),
      true,
    ),
    CheckoutState::InProgress => (
      ctx.t_args("merges.page.properties.inProgress", [("branch", branch)]),
      true,
    ),
    CheckoutState::OtherBranch(_) | CheckoutState::Beside(_) | CheckoutState::Unknown => (
      ctx.t_args("merges.page.properties.onBranch", [("branch", branch)]),
      false,
    ),
  }
}

/// A fact: its key, and its value, which is `warning` when files are changed and gives way at its end when the room
/// is short: a long folder is cut rather than push *Runner log folder* out of the window.
fn fact(id: &str, key: &str, value: &str, warning: bool) -> Element {
  let shrink = if id.ends_with("folder") { 1.0 } else { 0.0 };
  Row::new()
    .id(id.to_owned())
    .min_width(Dimension::Px(0.0))
    .flex_shrink(shrink)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(key)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(ExtraTypography::PropertyMono.role())
        .color(if warning {
          PaletteColor::Warning
        } else {
          PaletteColor::TextSecondary
        })
        .min_width(Dimension::Px(0.0))
        .flex_shrink(shrink)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// A link: its words in 12/500 `text_primary` and an arrow after them, 4 apart (`Link · Runner log folder`).
pub(super) fn link(id: &str, (arrow, words): (Icon, &str), press: Press) -> Element {
  pressed(press)
    .id(id.to_owned())
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("a11y_name", words.to_owned())
    .child(
      Text::new(words)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(icon(arrow, IconSize::Base, PaletteColor::TextPrimary))
    .into()
}
