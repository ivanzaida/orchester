//! Modules (ORC-48, redesigned in ORC-112 to `Modules · redesign` `oEELJ` of the new `orchester.pen`): the Modules list
//! on its views Open and Closed ([`list`]), a module's page in the task page's frame with its tabs Summary
//! ([`summary`]) and Burndown ([`burndown`]), and New module and Edit module as pages in the task form's style
//! ([`form`]). The data and the rules are `orchester_workspace::modules`; the dates are picked with the UI kit's date
//! picker (`ui::date_picker`).

mod burndown;
mod chart;
mod form;
mod form_base;
mod form_chips;
mod header;
mod list;
mod list_row;
mod more;
mod page;
mod properties;
mod reading;
mod save;
mod summary;
mod tasks;
pub(crate) mod words;

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ModuleId;

use crate::{
  project::{ShownProject, TasksState},
  routes::{ModuleListTab, ModuleTab},
  ui::{
    icon::Icon,
    message::message_text,
    skeleton::{self, Skeleton},
    state::{EmptyState, load_error},
  },
};

pub fn list(ctx: &mut Ctx, project: &ShownProject, tab: ModuleListTab) -> Element {
  let key = format!("modules-{}", project.open.project.id());
  ctx.mount_keyed::<list::ModulesList>(
    &key,
    list::ListProps {
      project: project.clone(),
      tab,
    },
  )
}

/// Keyed by the module, not the tab: the frame stays as the tabs switch.
pub fn page(ctx: &mut Ctx, project: &ShownProject, module: ModuleId, tab: ModuleTab) -> Element {
  let key = format!("module-{}-{module}", project.open.project.id());
  ctx.mount_keyed::<page::ModuleScreen>(
    &key,
    page::PageProps {
      project: project.clone(),
      module,
      tab,
    },
  )
}

pub fn new_module(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let key = format!("module-form-new-{}", project.open.project.id());
  ctx.mount_keyed::<form::ModuleFormPage>(
    &key,
    form::FormProps {
      project: project.clone(),
      module: None,
    },
  )
}

/// Edit module, once the project's modules are read; until then, or when they cannot be or the module is gone, the
/// page says so.
pub fn edit_module(ctx: &mut Ctx, project: &ShownProject, module: ModuleId) -> Element {
  let found = match project.feed.tasks.get() {
    TasksState::Ready(tasks) => tasks.modules.iter().find(|stored| stored.id() == module).cloned(),
    TasksState::Loading => {
      // No loading frame draws Edit module: its fields' bones stand in one outline until the module is read.
      let bones = skeleton::list(
        ctx,
        "module-form-reading",
        &[(60.0, 320.0), (90.0, 420.0), (80.0, 160.0)],
      );
      return screen(Skeleton::new("module-loading", ctx.t("modules.page.reading")).show(ctx, bones));
    }
    TasksState::Failed(error) => {
      let feed = project.feed.clone();
      let words = message_text(ctx, &error);
      return screen(load_error(
        ctx,
        "module-load-error",
        (&ctx.t("modules.readFailed"), &words),
        move || feed.saved(),
      ));
    }
  };
  let Some(found) = found else {
    let missing = EmptyState::new(
      "module-not-found",
      Icon::SearchX,
      ctx.t("modules.page.notFound"),
      ctx.t("modules.page.notFoundBody"),
    )
    .unframed();
    return screen(missing.into());
  };
  // Keyed by what the form was filled from: a module saved elsewhere meanwhile fills a new form only when it is
  // opened again, and a save over it is refused as made meanwhile.
  let key = format!("module-form-{}-{module}", project.open.project.id());
  ctx.mount_keyed::<form::ModuleFormPage>(
    &key,
    form::FormProps {
      project: project.clone(),
      module: Some(Arc::new(found)),
    },
  )
}

/// `content` as Edit module's screen while there is no form to show.
fn screen(content: Element) -> Element {
  Column::new()
    .id("screen-modules")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(page::body_padding())
    .child(content)
    .into()
}
