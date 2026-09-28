//! Extra writable folders on the Permissions tab (implemented, not drawn yet; built from the
//! allowed commands of 04.04 `T1rjv`: the field `mwPz8` and *Add command*, then the entries
//! `bpmzN`, one per line and as wide as the column since paths are long, and the notice with an
//! action of Project settings › Environment `CMSyB`). Folders outside the
//! task worktree that an agent which edits files may also write to; each is checked when it is
//! added (`crate::agents::folders`). When Environment sets `CARGO_TARGET_DIR` and the profile does
//! not list it yet, a notice offers to add it; nothing is added until the person does.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::permission::{RepositoryAccess, WritableFolders};

use super::editor::ProfileEditor;
use crate::{
  agents::folders::{lists, with_folder, without_folder},
  environment::CARGO_TARGET_DIR,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    command_chip::CommandChip,
    field::{text_input, wrapping_field_error},
    form::form,
    icon::Icon,
    message_text,
    notice::Notice,
  },
};

/// Folder field width: a path needs more room than a command (`mwPz8` is 180), the height of the
/// entries beside it (`bpmzN`), and of a notice's action (`IgW07` in `CMSyB`).
const FOLDER_INPUT_WIDTH: f32 = 280.0;
const FOLDER_HEIGHT: f32 = 26.0;
const NOTICE_ACTION_HEIGHT: f32 = 32.0;

impl ProfileEditor {
  /// The extra writable folders, a field to add one, what they mean, and the suggestion to add
  /// the build folder Environment sets.
  pub(super) fn folders_setting(&self, ctx: &Ctx) -> Element {
    let permissions = self.permissions.get();
    let folders = permissions.writable_folders().clone();
    let entries = folders.entries().iter().enumerate().map(|(index, folder)| {
      let (permissions, removed) = (self.permissions.clone(), folder.clone());
      let chip = CommandChip::new(format!("agent-folder-{index}"), folder.as_str().into()).on_remove(move || {
        permissions.update(|permissions| {
          let rest = without_folder(permissions.writable_folders(), &removed);
          permissions.set_writable_folders(rest);
        });
      });
      Element::from(chip.fill())
    });
    let add = self.add_folder();
    let input = text_input(
      ctx,
      "agent-new-folder",
      self.new_folder.clone(),
      self.folder_error.get().is_some(),
      ExtraTypography::MonoValue.role(),
    )
    .size(Dimension::Px(FOLDER_INPUT_WIDTH), Dimension::Px(FOLDER_HEIGHT))
    .placeholder(&ctx.t("agents.permissions.folderPlaceholder"));
    let add_button = ActionButton::new(ctx.t("agents.permissions.addFolder"))
      .icon(Icon::Plus)
      .compact()
      .id("agent-add-folder")
      .in_form()
      .height(Dimension::Px(FOLDER_HEIGHT));
    let add_button = match &add {
      Some(add) => {
        let add = add.clone();
        add_button.on_click(move || add())
      }
      None => add_button.on_click(|| {}).enabled(false),
    };
    // The field first, so it keeps its place (and the focus) as folders are added; then one folder
    // per line: paths are long, and a long one ends in an ellipsis before its remove icon.
    let row = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .overflow_visible()
          .child(input)
          .child(add_button.render(ctx)),
      )
      .with_children(entries);
    let note: Element = match self.folder_error.get() {
      Some(error) => wrapping_field_error(&message_text(ctx, &error)),
      None if permissions.repository() == RepositoryAccess::ReadOnly => hint(ctx, "agents.permissions.foldersReadOnly"),
      None if folders.is_empty() => hint(ctx, "agents.permissions.noFolders"),
      None => hint(ctx, "agents.permissions.foldersMeaning"),
    };
    let suggestion = (permissions.repository() == RepositoryAccess::Write)
      .then(|| self.target_suggestion(ctx, &folders))
      .flatten();
    Column::new()
      .id("agent-folders")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .overflow_visible()
      .child(Text::new(&ctx.t("agents.permissions.folders")).variant(TypographyStyle::Label))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .overflow_visible()
          // Enter in the folder field adds the folder rather than saving the draft.
          .child(form(add, row))
          .child(note),
      )
      .with_children(suggestion)
      .into()
  }

  /// Adds the typed folder, or says why it cannot be added; `None` while nothing is typed.
  fn add_folder(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
    if self.new_folder.get().trim().is_empty() {
      return None;
    }
    let (typed, permissions, error) = (
      self.new_folder.clone(),
      self.permissions.clone(),
      self.folder_error.clone(),
    );
    let repo_root = self.context.project.repo_root.clone();
    Some(Arc::new(move || {
      let list = permissions.get_untracked().writable_folders().clone();
      match with_folder(&list, &typed.get_untracked(), &repo_root) {
        Ok(list) => {
          permissions.update(|permissions| permissions.set_writable_folders(list));
          typed.set(String::new());
          error.set(None);
        }
        Err(message) => error.set(Some(message)),
      }
    }))
  }

  /// *Allow agents to write to CARGO_TARGET_DIR* while Environment sets it and the profile does not
  /// list it yet.
  fn target_suggestion(&self, ctx: &Ctx, folders: &WritableFolders) -> Option<Element> {
    let target = self.context.agents.target_dir.clone()?;
    let repo_root = self.context.project.repo_root.clone();
    if lists(folders, &target, &repo_root) {
      return None;
    }
    let (permissions, error) = (self.permissions.clone(), self.folder_error.clone());
    let added = suggested_entry(&target, &repo_root);
    let add = ActionButton::new(ctx.t("agents.permissions.addTargetFolder"))
      .icon(Icon::Plus)
      .id("agent-add-target-folder")
      .in_form()
      .height(Dimension::Px(NOTICE_ACTION_HEIGHT))
      .on_click(move || {
        let list = permissions.get_untracked().writable_folders().clone();
        match with_folder(&list, &added, &repo_root) {
          Ok(list) => {
            permissions.update(|permissions| permissions.set_writable_folders(list));
            error.set(None);
          }
          Err(message) => error.set(Some(message)),
        }
      })
      .render(ctx);
    Some(
      Notice::new(
        Icon::Lightbulb,
        ctx.t_args("agents.permissions.targetTitle", [("variable", CARGO_TARGET_DIR)]),
        ctx.t_args(
          "agents.permissions.targetDetail",
          [("variable", CARGO_TARGET_DIR), ("folder", &*target)],
        ),
      )
      .action(add)
      .id("agent-target-folder")
      .into(),
    )
  }
}

/// The folder `target` as the profile lists it: relative to the repository root when it lies inside
/// the repository, so the profile means the same folder in another checkout.
fn suggested_entry(target: &str, repo_root: &Path) -> String {
  match Path::new(target).strip_prefix(repo_root) {
    Ok(inside) if !inside.as_os_str().is_empty() => inside
      .components()
      .map(|part| part.as_os_str().to_string_lossy())
      .collect::<Vec<_>>()
      .join("/"),
    _ => target.to_owned(),
  }
}

fn hint(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::Hint.role())
    .color(PaletteColor::TextMuted)
    .into()
}
