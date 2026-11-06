//! Review (`Review - v2` `j1Drb`): the tasks whose run finished, and a task's review. Review is no longer a hub of
//! tabs: the list is [`list`] (ORC-794); the review page (ORC-795) is built in its own task, so until then a task's
//! route says that Review is being rebuilt and shows nothing else. The Merge queue, Pull requests and Merges keep their
//! own routes until their tasks remove them.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::project::TaskKey;

use super::placeholder::centered;
use crate::ui::icon::Icon;

mod list;

pub(super) use self::list::screen as list;

/// The review of the task `key` at `/project/<id>/review/<key>`.
pub(super) fn task(ctx: &mut Ctx, key: &TaskKey) -> Element {
  unavailable(ctx, key)
}

/// What a task's route shows: that Review is being rebuilt, in the screen's own element, `screen-review`.
fn unavailable(ctx: &Ctx, key: &TaskKey) -> Element {
  let title = ctx.t_args("review.unavailable.taskTitle", [("key", key.to_string())]);
  centered(Icon::GitPullRequest, &title, &ctx.t("review.unavailable.body"))
    .id("screen-review")
    .into()
}
