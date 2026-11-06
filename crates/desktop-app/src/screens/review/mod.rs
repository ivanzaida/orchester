//! Review (`Review - v2` `j1Drb`): the tasks whose run finished, and a task's review. Review is no longer a hub of
//! tabs: the list (ORC-794) and the review page (ORC-795) are built in their own tasks, so until then both routes
//! say that Review is being rebuilt and show nothing else. The Merge queue, Pull requests and Merges keep their own
//! routes until their tasks remove them.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::project::TaskKey;

use super::placeholder::centered;
use crate::ui::icon::Icon;

/// Review's list at `/project/<id>/review`.
pub(super) fn list(ctx: &mut Ctx) -> Element {
  unavailable(ctx, None)
}

/// The review of the task `key` at `/project/<id>/review/<key>`.
pub(super) fn task(ctx: &mut Ctx, key: &TaskKey) -> Element {
  unavailable(ctx, Some(key))
}

/// What both routes show: that Review is being rebuilt, in the screen's own element, `screen-review`.
fn unavailable(ctx: &Ctx, key: Option<&TaskKey>) -> Element {
  let title = match key {
    Some(key) => ctx.t_args("review.unavailable.taskTitle", [("key", key.to_string())]),
    None => ctx.t("review.unavailable.title"),
  };
  centered(Icon::GitPullRequest, &title, &ctx.t("review.unavailable.body"))
    .id("screen-review")
    .into()
}
