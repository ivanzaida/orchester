//! What a link's tooltip says (`Tooltip · link destination` in `agv8r`): the place a link in the app opens and that it
//! opens in this tab (*ORC-24 › Conversation · opens in this tab*), the project first when it is another than the
//! conversation's (*Atlas › ATL-9 · …*); for a link to the web, the address it opens in the browser.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;

use super::{super::data::ProjectNames, link::Target};
use crate::routes::{ProjectPage, Route, TaskTab};

/// The tooltip of a link to `target`, in a conversation of `project`; `names` are the projects'.
pub(super) fn tip(ctx: &mut Ctx, target: &Target, (project, names): (ProjectId, &ProjectNames)) -> Arc<str> {
  match target {
    Target::App(route) => {
      let place = place(ctx, route, (project, names));
      ctx.t_args("orchestrator.link.opensHere", [("place", place.as_str())])
    }
    Target::Web(url) => ctx.t_args("orchestrator.link.opensBrowser", [("url", url.as_str())]),
  }
}
/// The place `route` names: a task by its key and the tab it opens on, any other page by its screen's name; after
/// the project's name when the place is in another project.
fn place(ctx: &mut Ctx, route: &Route, (project, names): (ProjectId, &ProjectNames)) -> String {
  let page = match route {
    Route::Project(_, ProjectPage::Task(key, view)) => match view.tab() {
      TaskTab::Summary => key.to_string(),
      tab => ctx
        .t_args(
          "orchestrator.link.taskTab",
          [("key", key.to_string()), ("tab", ctx.t(tab.title_key()).to_string())],
        )
        .to_string(),
    },
    route => ctx.t(route.screen().title_key()).to_string(),
  };
  let other = route.project().filter(|other| *other != project);
  match other.and_then(|other| names.get(other)) {
    Some(name) => ctx
      .t_args(
        "orchestrator.link.inProject",
        [("project", name), ("page", page.as_str())],
      )
      .to_string(),
    None => page,
  }
}
