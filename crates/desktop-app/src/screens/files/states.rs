//! What stands for the file (`State area` `rTYfb`: a 500 wide state card, 96 under the file's head, on
//! `code_bg`): no file chosen with *Go to file…* (`o3iVXj`), a path the page refuses and why (`S3Py8v`), a
//! binary (`c2jICC`) or too large file (`ISM2O`), a file that is not in the version or cannot be read; and,
//! in place of the tree and the file, a version that no longer exists with the checkout's version and the
//! base branch to show instead (`SyXwU`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::{byte_size, short_revision},
  files::{FilePath, FilesError, MAX_VIEW_BYTES, PathError, Version},
};

use super::{Page, reads::Failure};
use crate::{
  theme::ExtraColor,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    state::EmptyState,
  },
};

/// The card's width and its room above it (`zwcCY` in `rTYfb`).
const CARD_WIDTH: f32 = 500.0;
const CARD_TOP: f32 = 96.0;

/// `state` in the room of the file, 96 under its head and centered.
fn area(state: EmptyState) -> Element {
  Column::new()
    .id("files-state")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::horizontal(SpacingSize::Section))
    .align_items(Alignment::Center)
    .background(ExtraColor::CodeBackground.role())
    .child(Row::new().height(Dimension::Px(CARD_TOP)))
    .child(
      Column::new()
        .width(Dimension::Px(CARD_WIDTH))
        .max_width(Dimension::Pct(100.0))
        .child(state.wide()),
    )
    .into()
}

/// No file chosen: pick one in the tree, or find it by name.
pub fn no_file(ctx: &mut Ctx, page: &Page<'_>) -> Element {
  let goto = page.view.goto.clone();
  area(
    EmptyState::new(
      "files-no-file",
      Icon::Files,
      ctx.t("files.noFile.title"),
      ctx.t("files.noFile.detail"),
    )
    .action(
      ActionButton::new(
        "files-no-file-go-to",
        ButtonKind::Secondary,
        ctx.t("files.goTo.trigger"),
      )
      .icon(Icon::Search)
      .on_click(move || goto.show()),
    ),
  )
}

/// The route names a path the page does not open, and why.
pub fn refused(ctx: &Ctx, page: &Page<'_>, error: &PathError) -> Element {
  let (title, detail) = match error {
    PathError::Absolute(path) | PathError::Parent(path) | PathError::Invalid(path) | PathError::Outside(path) => (
      ctx.t("files.refused.title"),
      ctx.t_args(
        "files.refused.detail",
        [
          ("path", path.as_str()),
          ("folder", &page.project.open.repo_root.display().to_string()),
        ],
      ),
    ),
    PathError::Hidden(_) | PathError::Empty => (
      ctx.t("files.refused.hiddenTitle"),
      message_text(ctx, &FilesError::Path(error.clone()).message()),
    ),
  };
  area(EmptyState::new("files-refused", Icon::ShieldX, title, detail))
}

/// The file is not in the version, is a folder, or cannot be read.
pub fn unreadable(ctx: &Ctx, failure: &Failure) -> Element {
  let title = match failure {
    Failure::Files(FilesError::Missing(_)) => "files.missing.title",
    _ => "files.unreadable.title",
  };
  area(EmptyState::new(
    "files-unreadable",
    Icon::FileWarning,
    ctx.t(title),
    message_text(ctx, &failure.message()),
  ))
}

/// A binary file is not shown.
pub fn binary(ctx: &Ctx, file: &FilePath) -> Element {
  area(EmptyState::new(
    "files-binary",
    Icon::Binary,
    ctx.t("files.binary.title"),
    ctx.t_args("files.binary.detail", [("name", file.name())]),
  ))
}

/// A file of `size` bytes, above what the page shows.
pub fn too_large(ctx: &Ctx, file: &FilePath, size: u64) -> Element {
  let size = message_text(ctx, &byte_size(size));
  let limit = message_text(ctx, &byte_size(MAX_VIEW_BYTES));
  area(EmptyState::new(
    "files-too-large",
    Icon::FileWarning,
    ctx.t("files.tooLarge.title"),
    ctx.t_args(
      "files.tooLarge.detail",
      [
        ("name", file.name()),
        ("size", size.as_ref()),
        ("limit", limit.as_ref()),
      ],
    ),
  ))
}

/// "Binary · 48.2 KB" at the end of the head of a file that is not shown.
pub fn size_note(ctx: &Ctx, kind: &str, size: u64) -> String {
  [ctx.t(kind).to_string(), message_text(ctx, &byte_size(size)).to_string()].join(&ctx.t("files.meta.separator"))
}

/// The version shown no longer exists: in place of the tree and the file, why, with the checkout's version
/// and the base branch to show instead.
pub fn version_gone(ctx: &mut Ctx, page: &Page<'_>) -> Element {
  let (title, detail) = match page.at() {
    Version::Run(_) => match page.run_facts() {
      Some(run) => (
        ctx
          .t_args("files.gone.worktree", [("number", run.number.to_string())])
          .to_string(),
        ctx.t("files.gone.worktreeDetail"),
      ),
      None => (
        ctx.t("files.gone.worktreeUnnamed").to_string(),
        ctx.t("files.gone.worktreeDetail"),
      ),
    },
    Version::Revision(revision) => (
      ctx
        .t_args("files.gone.revision", [("revision", short_revision(revision.as_str()))])
        .to_string(),
      ctx.t("files.gone.revisionDetail"),
    ),
    Version::Base | Version::Checkout => (ctx.t("files.gone.base").to_string(), ctx.t("files.gone.baseDetail")),
  };
  let mut state = EmptyState::new("files-gone", Icon::FolderX, title, detail).action(
    ActionButton::new(
      "files-gone-checkout",
      ButtonKind::Primary,
      ctx.t("files.gone.showCheckout"),
    )
    .icon(Icon::Folder)
    .on_click(page.switch_to(ctx, Version::Checkout)),
  );
  let has_base = page.choices().iter().any(|choice| choice.version == Version::Base);
  if has_base && *page.at() != Version::Base {
    state = state.action(
      ActionButton::new("files-gone-base", ButtonKind::Secondary, ctx.t("files.gone.showBase"))
        .icon(Icon::GitCommitHorizontal)
        .on_click(page.switch_to(ctx, Version::Base)),
    );
  }
  area(state)
}
