//! Review (`Review - v2` `j1Drb`): the tasks whose run finished, and a task's review. Review is no longer a hub of
//! tabs: the list (ORC-794) is built in its own task, so until then its route says that Review is being rebuilt and
//! shows nothing else; a task's review is [`task`] (ORC-795). The Merge queue, Pull requests and Merges keep their own
//! routes until their tasks remove them.

pub(super) mod task;

use lurq::{app::ctx::Ctx, node::Element};

use super::placeholder::centered;
use crate::ui::icon::Icon;

/// Review's list at `/project/<id>/review`: that Review is being rebuilt, in the screen's own element,
/// `screen-review`.
pub(super) fn list(ctx: &mut Ctx) -> Element {
  centered(
    Icon::GitPullRequest,
    &ctx.t("review.unavailable.title"),
    &ctx.t("review.unavailable.body"),
  )
  .id("screen-review")
  .into()
}
