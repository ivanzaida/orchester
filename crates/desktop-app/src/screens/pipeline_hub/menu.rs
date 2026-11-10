//! The ⋯ in a pipeline's head (`Pipeline menu` `x6ZrKL` in `Pipelines · v2` P11 `nrTt0`): *Rename*, which makes the
//! title editable in place, *Duplicate*, which opens New pipeline filled from this one, *Make default for New task* (on
//! the default pipeline *Default for New task* with a check, which does nothing more), a separator and *Archive* with
//! how many tasks use the pipeline. A pipeline that was never published cannot be copied or made the default, and its
//! rows say so.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  components::Column,
  core::Signal,
  node::dimension::Dimension,
};
use orchester_core::id::ProjectId;
use orchester_workspace::messages::Message;

use super::model::Entry;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  ui::{
    icon::Icon,
    menu::{MenuItem, menu_panel, separator},
  },
};

/// The menu's width (`Pipeline menu` `x6ZrKL`).
const MENU_WIDTH: f32 = 230.0;

/// What the rows do.
pub(super) struct Actions {
  pub rename: Arc<dyn Fn() + Send + Sync>,
  pub make_default: FutureAction<(), (), Message>,
  pub archive: Arc<dyn Fn() + Send + Sync>,
}

/// The menu's rows for `entry`; a choice closes the menu `open` shows.
pub(super) fn panel(
  ctx: &mut Ctx,
  (project, entry, default): (ProjectId, &Entry, bool),
  open: &Signal<bool>,
  actions: &Actions,
) -> Column {
  let rename = actions.rename.clone();
  let rename = MenuItem::new(
    "pipeline-hub-menu-rename",
    ctx.t("pipeline_hub.menu.rename"),
    move || rename(),
  )
  .icon(Icon::Pencil)
  .render(ctx, open);
  let duplicate = duplicate(ctx, (project, entry)).render(ctx, open);
  let make_default = make_default(ctx, entry, default, actions).render(ctx, open);
  let archive = actions.archive.clone();
  let used = match entry.used_by {
    0 => None,
    1 => Some(ctx.t("pipeline_hub.menu.usedOne")),
    count => Some(ctx.t_args("pipeline_hub.menu.used", [("count", count.to_string())])),
  };
  let archive = MenuItem::new(
    "pipeline-hub-menu-archive",
    ctx.t("pipeline_hub.menu.archive"),
    move || archive(),
  )
  .icon(Icon::Archive);
  let archive = match used {
    Some(used) => archive.hint(used),
    None => archive,
  }
  .render(ctx, open);
  let separator = separator(ctx);
  menu_panel("pipeline-hub-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .child(rename)
    .child(duplicate)
    .child(make_default)
    .child(separator)
    .child(archive)
}

fn duplicate(ctx: &mut Ctx, (project, entry): (ProjectId, &Entry)) -> MenuItem {
  let id = "pipeline-hub-menu-duplicate";
  let label = ctx.t("pipeline_hub.menu.duplicate");
  let item = if entry.is_draft() {
    MenuItem::unavailable(id, label, ctx.t("pipeline_hub.menu.duplicateDraft"))
  } else {
    let copy = Route::Project(project, ProjectPage::CopyPipeline(entry.id));
    MenuItem::opens(id, label, Navigation::of(ctx).to(copy))
  };
  item.icon(Icon::Copy)
}

fn make_default(ctx: &mut Ctx, entry: &Entry, default: bool, actions: &Actions) -> MenuItem {
  let id = "pipeline-hub-menu-default";
  let item = if default {
    MenuItem::new(id, ctx.t("pipeline_hub.menu.isDefault"), || {}).chosen(true)
  } else if entry.is_draft() {
    MenuItem::unavailable(
      id,
      ctx.t("pipeline_hub.menu.makeDefault"),
      ctx.t("pipeline_hub.menu.makeDefaultDraft"),
    )
  } else {
    let action = actions.make_default.clone();
    MenuItem::new(id, ctx.t("pipeline_hub.menu.makeDefault"), move || action.run(()))
  };
  item.icon(Icon::Star)
}
