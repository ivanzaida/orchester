//! The Summary's top card while the run's agent waits for a decision (`idBwz` in `G0rzJy`): who waits
//! for what, since when, and *Review request*, which opens the approval dialog on it.

use lurq::{app::ctx::Ctx, node::Element};

use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, pages::Shown},
  tasks::{approvals::ShownApproval, detail::RunView},
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// The oldest request `run` waits on, as the project's tasks know it.
pub(super) fn waiting_request(ctx: &mut Ctx, run: &RunView) -> Option<ShownApproval> {
  let shown = ctx.use_context::<Shown>()?;
  let tasks = shown.tasks.ready()?;
  tasks
    .pending_tool_approvals
    .iter()
    .find(|pending| pending.approval.run == run.id())
    .cloned()
}

/// The card of the oldest request `run` waits on; `None` when it waits on none.
pub(super) fn review_card(ctx: &mut Ctx, run: &RunView) -> Option<Element> {
  let waiting = waiting_request(ctx, run)?;
  let queue = ctx.use_context::<ApprovalQueue>()?;
  let approval = &waiting.approval;
  let title = if approval.command {
    ctx.t_args(
      "runs.summary.approval.titleCommand",
      [("agent", approval.agent.as_str())],
    )
  } else {
    ctx.t_args(
      "runs.summary.approval.titleTool",
      [
        ("agent", approval.agent.as_str()),
        ("server", approval.server.as_str()),
        ("tool", approval.action.as_str()),
      ],
    )
  };
  let detail = ctx.t_args(
    "runs.summary.approval.detail",
    [("time", display::clock(approval.requested_at).as_str())],
  );
  let id = approval.id.clone();
  let review = ActionButton::new(ctx.t("runs.summary.approval.review"))
    .id("task-review-request")
    .icon(Icon::Hand)
    .on_click(move || queue.review(&id))
    .render(ctx);
  Some(
    Notice::new(Icon::Hand, title, detail)
      .emphasized()
      .id("task-approval-card")
      .action(review)
      .into(),
  )
}
