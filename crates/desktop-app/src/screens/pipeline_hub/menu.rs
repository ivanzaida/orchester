//! The ⋯ in a pipeline's head (`Pipeline menu` `x6ZrKL` in `Pipelines · v2` P11 `nrTt0`): *Rename*, which makes the
//! title editable in place, *Duplicate*, which opens New pipeline filled from this one, *Make default for New task* (on
//! the default pipeline *Default for New task* with a check, which does nothing more), a separator and *Archive* with
//! how many tasks use the pipeline. Every pipeline has a published version (D-032), so each row is always available.

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
  /// None in a team project: the default pipeline is kept on this computer, which keeps no copy of the team's.
  pub make_default: Option<FutureAction<(), (), Message>>,
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
  let make_default = actions
    .make_default
    .clone()
    .map(|action| make_default(ctx, default, action).render(ctx, open));
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
    .with_children(make_default)
    .child(separator)
    .child(archive)
}

fn duplicate(ctx: &mut Ctx, (project, entry): (ProjectId, &Entry)) -> MenuItem {
  let id = "pipeline-hub-menu-duplicate";
  let label = ctx.t("pipeline_hub.menu.duplicate");
  let copy = Route::Project(project, ProjectPage::CopyPipeline(entry.id));
  MenuItem::opens(id, label, Navigation::of(ctx).to(copy)).icon(Icon::Copy)
}

fn make_default(ctx: &Ctx, default: bool, action: FutureAction<(), (), Message>) -> MenuItem {
  let id = "pipeline-hub-menu-default";
  let item = if default {
    MenuItem::new(id, ctx.t("pipeline_hub.menu.isDefault"), || {}).chosen(true)
  } else {
    MenuItem::new(id, ctx.t("pipeline_hub.menu.makeDefault"), move || action.run(()))
  };
  item.icon(Icon::Star)
}
