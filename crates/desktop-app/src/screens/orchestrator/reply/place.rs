//! What the tooltip of a link to a place in the app says (`Tooltip · link destination` in `agv8r`): the place and
//! that it opens in this tab (*ORC-24 › Conversation · opens in this tab*), the project first when it is another
//! than the conversation's (*Atlas › ATL-9 · …*).

use std::sync::Arc;

use lurq::app::i18n::I18n;
use orchester_core::id::ProjectId;

use super::super::data::ProjectNames;
use crate::routes::{Route, TaskTab};

/// The tooltip of a link to `route`, in a conversation of `project`; `names` are the projects'.
pub(super) fn tip(i18n: &I18n, route: &Route, (project, names): (ProjectId, &ProjectNames)) -> Arc<str> {
  let place = place(i18n, route, (project, names));
  i18n.t_args("orchestrator.link.opensHere", [("place", place.as_str())])
}

/// The place `route` names: a task by its key and the tab it opens on, any other page by its screen's name; after
/// the project's name when the place is in another project.
fn place(i18n: &I18n, route: &Route, (project, names): (ProjectId, &ProjectNames)) -> String {
  let task = match route {
    Route::Project(_, page) => page.task_view(),
    _ => None,
  };
  let page = match (task, route) {
    (Some((key, view)), _) => match view.tab() {
      TaskTab::Summary => key.to_string(),
      tab => i18n
        .t_args(
          "orchestrator.link.taskTab",
          [("key", key.to_string()), ("tab", i18n.t(tab.title_key()).to_string())],
        )
        .to_string(),
    },
    (None, route) => i18n.t(route.screen().title_key()).to_string(),
  };
  let other = route.project().filter(|other| *other != project);
  match other.and_then(|other| names.get(other)) {
    Some(name) => i18n
      .t_args(
        "orchestrator.link.inProject",
        [("project", name), ("page", page.as_str())],
      )
      .to_string(),
    None => page,
  }
}
