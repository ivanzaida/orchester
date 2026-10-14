//! *Open project folder…* (`Open project folder…` in `S5HDKf` and `UQZRa`): the system's folder picker, then
//! the project the chosen folder keeps, which opens on its Overview and is listed again if it was removed.
//! A folder that keeps no project is refused as the switcher's *Open folder…* refuses it; the error line
//! names the folder and stays until the next attempt (`FV5pQ` in `kwpO2`).

use std::{path::PathBuf, sync::Arc};

use lurq::app::{
  component::DevtoolsInspectable,
  ctx::{Ctx, FutureAction},
};
use orchester_workspace::{background, messages::Message, workspace::OpenProject};
use parking_lot::Mutex;

use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  shell::sidebar::switcher::{FolderRefusal, open_folder},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

/// The picker's title; what the chosen folder held.
pub(super) type OpenAction = FutureAction<Arc<str>, Opened, Message>;

/// What *Open project folder…* found.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Opened {
  /// The picker was closed without a folder.
  Closed,
  Project(OpenProject),
  Refused(PathBuf, FolderRefusal),
}

impl DevtoolsInspectable for Opened {}

pub(super) fn open_action(ctx: &mut Ctx, services: &Services) -> OpenAction {
  let picker = PathPicker::of(ctx);
  let workspace = services.workspace.clone();
  ctx.future_action(move |title: Arc<str>| {
    let (picker, workspace) = (picker.clone(), workspace.clone());
    async move {
      let Some(folder) = picker.pick(PathKind::Folder, title).await else {
        return Ok(Opened::Closed);
      };
      let chosen = folder.clone();
      let opened = background::run(move || open_folder(&workspace, &chosen)).await?;
      Ok(match opened {
        Ok(project) => Opened::Project(project),
        Err(refusal) => Opened::Refused(folder, refusal),
      })
    }
  })
}

/// Opens the project a chosen folder keeps on its Overview; follows `open` once per screen.
pub(super) fn follow(ctx: &mut Ctx, open: &OpenAction, followed: &Mutex<bool>) {
  let mut followed = followed.lock();
  if *followed {
    return;
  }
  *followed = true;
  let navigation = Navigation::of(ctx);
  ctx.watch(&open.state(), move |state| {
    if state.is_pending() || !state.is_fulfilled() {
      return;
    }
    if let Some(Opened::Project(project)) = &state.data {
      navigation.go(Route::Project(
        project.project.id(),
        ProjectPage::Overview(OverviewTab::default()),
      ));
    }
  });
}

/// *Open project folder…*, waiting while the picker is open or the folder is read.
pub(super) fn button(ctx: &Ctx, open: &OpenAction, id: &str) -> ActionButton {
  let button =
    ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t("projects.openFolder")).icon(Icon::FolderOpen);
  if open.state().get().is_pending() {
    return button.loading();
  }
  let (open, title) = (open.clone(), ctx.t("projects.openFolderTitle"));
  button.on_click(move || open.run(title.clone()))
}

/// Why the last *Open project folder…* did not open a project, naming the folder; `None` once a project
/// opened, while the next attempt runs, and before the first.
pub(super) fn failure(ctx: &Ctx, open: &OpenAction) -> Option<Arc<str>> {
  let state = open.state().get();
  if state.is_pending() {
    return None;
  }
  // Reading the chosen folder stopped before it ended.
  if let Some(reason) = state.error.as_ref().filter(|_| state.is_rejected()) {
    return Some(message_text(ctx, reason));
  }
  let Some(Opened::Refused(folder, refusal)) = state.data else {
    return None;
  };
  let path = folder.display().to_string();
  Some(match refusal {
    FolderRefusal::NoProject => ctx.t_args("projects.errors.noProject", [("path", path)]),
    FolderRefusal::NotRepository => ctx.t_args("projects.errors.notRepository", [("path", path)]),
    FolderRefusal::NotTopFolder { top } => ctx.t_args(
      "projects.errors.notTopFolder",
      [("path", path), ("top", top.display().to_string())],
    ),
    FolderRefusal::Newer { .. } => ctx.t_args("projects.errors.newerData", [("path", path)]),
    // The reason is in the log (`open_folder`), as the line says.
    FolderRefusal::Unreadable(_) => ctx.t_args("projects.errors.notRead", [("path", path)]),
  })
}
