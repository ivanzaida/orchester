//! The version switch (`Files · version switch open` `kadDh`): a picker with the version shown, and its menu
//! (`Version menu` `DqtY6`, 340 wide) under `SHOW THE FILE FROM`: the project's checkout, the worktree of the
//! run the page was opened from while it exists, the base branch and a commit a link named, each with
//! where it is and what the file is there, the shown one checked; a note under them says that only
//! versions that exist are listed. A version that no longer exists keeps its name in the picker, marked
//! removed, in `warning` (`U34ugO` in `SyXwU`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display::{short_revision, shorten_path},
  files::{FileFacts, RunFacts, Version, VersionChoice},
};

use super::Page;
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel, separator},
    picker::Picker,
  },
};

/// The menu's width (`DqtY6`), and the characters of the checkout's folder its detail keeps on one line.
const MENU_WIDTH: f32 = 340.0;
pub const FOLDER_CHARS: usize = 36;

/// The icon of a version: a folder for the checkout, a branch for a run's worktree, a commit otherwise.
pub const fn glyph(version: &Version) -> Icon {
  match version {
    Version::Checkout => Icon::Folder,
    Version::Run(_) => Icon::GitBranch,
    Version::Base | Version::Revision(_) => Icon::GitCommitHorizontal,
  }
}

/// The name of the version shown: "Project checkout", "Run #12 worktree", "Base branch", "Commit 4d2b8e0".
pub fn label(ctx: &Ctx, page: &Page<'_>) -> String {
  name(ctx, page.at(), page.run_facts())
}

fn name(ctx: &Ctx, version: &Version, run: Option<&RunFacts>) -> String {
  match (version, run) {
    (Version::Checkout, _) => ctx.t("files.version.checkout").to_string(),
    (Version::Run(_), Some(run)) => ctx
      .t_args("files.version.run", [("number", run.number.to_string())])
      .to_string(),
    (Version::Run(_), None) => ctx.t("files.version.runUnnamed").to_string(),
    (Version::Base, _) => ctx.t("files.version.base").to_string(),
    (Version::Revision(revision), _) => ctx
      .t_args(
        "files.version.revision",
        [("revision", short_revision(revision.as_str()))],
      )
      .to_string(),
  }
}

/// The picker and, while it is open, its menu.
pub fn switch(ctx: &mut Ctx, page: &Page<'_>) -> (Element, Option<Popup>) {
  let view = page.view;
  let shown = label(ctx, page);
  let gone = page.versions.is_some() && page.shown_choice().is_none();
  let picker = Picker::new(
    "files-version",
    (
      glyph(page.at()),
      if gone {
        ctx
          .t_args("files.version.removed", [("version", shown.as_str())])
          .to_string()
      } else {
        shown
      },
    ),
    view.version_open.clone(),
    view.version_anchor.clone(),
  );
  let picker = if gone { picker.warning() } else { picker };
  let menu = (view.version_open.get() && page.versions.is_some()).then(|| {
    let mut panel = menu_panel("files-version-menu").width(Dimension::Px(MENU_WIDTH)).child(
      Column::new()
        .padding(
          Padding::new()
            .top(SpacingSize::Sm)
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .bottom(SpacingSize::Xs),
        )
        .child(Text::new(&ctx.t("files.version.heading")).variant(ExtraTypography::ColumnHeading.role())),
    );
    for choice in page.choices() {
      let item = option(ctx, page, choice);
      panel = panel.child(item.render(ctx, &view.version_open));
    }
    panel = panel.child(separator(ctx)).child(note(ctx, page));
    menu_at_end(view.version_anchor.clone(), view.version_open.clone(), panel)
  });
  (picker.into(), menu)
}

/// A version of the menu, with where it is and what the file is there.
fn option(ctx: &mut Ctx, page: &Page<'_>, choice: &VersionChoice) -> MenuItem {
  let id = format!("files-version:{}", choice.version);
  let label = name(ctx, &choice.version, page.run_facts());
  let mut parts = Vec::new();
  match &choice.version {
    Version::Checkout => {
      parts.push(shorten_path(&page.project.open.repo_root, FOLDER_CHARS));
      parts.extend(choice.branch.as_ref().map(ToString::to_string));
    }
    Version::Run(_) => parts.extend(page.run_facts().into_iter().flat_map(run_parts)),
    Version::Base | Version::Revision(_) => parts.extend(commit_line(ctx, choice)),
  }
  parts.extend(choice.file.map(|file| facts(ctx, file)));
  let switch = page.switch_to(ctx, choice.version.clone());
  MenuItem::new(id, label, switch)
    .icon(glyph(&choice.version))
    .detail(parts.join(&ctx.t("files.meta.separator")))
    .chosen(choice.version == *page.at())
}

/// What names a run beside its number: its task and its step ("ORC-24 · Implement", `qsFI3`).
pub fn run_parts(run: &RunFacts) -> Vec<String> {
  [Some(run.task.to_string()), run.step.clone()]
    .into_iter()
    .flatten()
    .collect()
}

/// "main at 4d2b8e0", or the commit alone.
fn commit_line(ctx: &Ctx, choice: &VersionChoice) -> Option<String> {
  let commit = choice.commit.as_ref()?;
  let revision = short_revision(commit.as_str());
  Some(match &choice.branch {
    Some(branch) => ctx
      .t_args(
        "files.version.branchAt",
        [("branch", branch.as_str()), ("revision", revision)],
      )
      .to_string(),
    None => revision.to_owned(),
  })
}

/// What the file is in a version: its lines, or why it is not shown.
pub fn facts(ctx: &Ctx, file: FileFacts) -> String {
  match file {
    FileFacts::Absent => ctx.t("files.version.notHere").to_string(),
    FileFacts::Text { lines, .. } => lines_text(ctx, lines),
    FileFacts::Binary { .. } => ctx.t("files.version.binary").to_string(),
    FileFacts::TooLarge { .. } => ctx.t("files.version.tooLarge").to_string(),
    FileFacts::Unreadable => ctx.t("files.version.unreadable").to_string(),
  }
}

/// "612 lines".
pub fn lines_text(ctx: &Ctx, lines: usize) -> String {
  let key = if lines == 1 {
    "files.meta.lines.one"
  } else {
    "files.meta.lines.other"
  };
  ctx.t_args(key, [("count", lines.to_string())]).to_string()
}

/// The note under the versions: which task the page was opened from, when it was.
fn note(ctx: &Ctx, page: &Page<'_>) -> Element {
  let task = page.run_facts().map(|run| run.task.to_string());
  let text = match task {
    Some(task) => ctx
      .t_args("files.version.noteFrom", [("task", task.as_str())])
      .to_string(),
    None => ctx.t("files.version.note").to_string(),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .child(
      Text::new(&text)
        .id("files-version-note")
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0)),
    )
    .into()
}
