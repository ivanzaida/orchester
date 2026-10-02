//! The Summary's top card while the run's agent waits for a decision (`idBwz` in `G0rzJy`): who waits
//! for what, since when, how many more requests wait in other runs, and *Review request*, which opens
//! the approval dialog on it. The step that waits says what it waits on in the run's progress
//! ([`waiting_on`]).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, pages::Shown},
  tasks::{approvals::ShownApproval, detail::RunView},
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// The requests the project waits on, oldest first, as its tasks know them.
fn pending(ctx: &mut Ctx) -> Vec<ShownApproval> {
  ctx
    .use_context::<Shown>()
    .and_then(|shown| shown.tasks.ready().map(|tasks| tasks.pending_tool_approvals.to_vec()))
    .unwrap_or_default()
}

/// The requests `run` waits on, oldest first.
pub(super) fn waiting_requests(ctx: &mut Ctx, run: &RunView) -> Vec<ShownApproval> {
  pending(ctx)
    .into_iter()
    .filter(|pending| pending.approval.run == run.id())
    .collect()
}

/// The oldest request `run` waits on.
pub(super) fn waiting_request(ctx: &mut Ctx, run: &RunView) -> Option<ShownApproval> {
  waiting_requests(ctx, run).into_iter().next()
}

/// The card of the oldest request `run` waits on; `None` when it waits on none.
pub(super) fn review_card(ctx: &mut Ctx, run: &RunView) -> Option<Element> {
  let all = pending(ctx);
  let waiting = all.iter().find(|pending| pending.approval.run == run.id())?.clone();
  let others = all.iter().filter(|pending| pending.approval.run != run.id()).count();
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
  let key = match others {
    0 => "runs.summary.approval.detail",
    1 => "runs.summary.approval.detailOthers.one",
    _ => "runs.summary.approval.detailOthers.other",
  };
  let detail = ctx.t_args(
    key,
    [
      ("time", display::clock(approval.requested_at).as_str()),
      ("count", others.to_string().as_str()),
    ],
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

/// What step `step` waits on, as its progress row says it (*Waiting for you · pencil batch_design ·
/// since 14:06*); `None` when no request of `waiting` is the step's.
pub(super) fn waiting_on(ctx: &Ctx, waiting: &[ShownApproval], step: &str) -> Option<Arc<str>> {
  let shown = waiting.iter().find(|shown| shown.step.as_deref() == Some(step))?;
  let approval = &shown.approval;
  let subject = if approval.command {
    let (line, more) = shown.first_line();
    if more {
      ctx.t_args("runs.toolApproval.firstLineMore", [("line", line)])
    } else {
      line.into()
    }
  } else {
    ctx.t_args(
      "runs.progress.toolSubject",
      [("server", approval.server.as_str()), ("tool", approval.action.as_str())],
    )
  };
  Some(ctx.t_args(
    "runs.progress.waitingOn",
    [
      ("subject", &*subject),
      ("time", display::clock(approval.requested_at).as_str()),
    ],
  ))
}
