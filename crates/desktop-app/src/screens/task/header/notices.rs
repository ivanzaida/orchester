//! The frame's notices, over the tabs so every tab shows them: the task could not be read again (the
//! page shows it as last read), the runner did not carry out a control (a refused Resume says that the
//! daily token budget is used), an earlier run of the task cannot be read, and the task is blocked.
//! The design draws none of them on the task page; they are the UI kit's notices.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::{messages::Message, tasks::detail::TaskDetail};

use super::controls::RunControl;
use crate::ui::{
  icon::Icon,
  message::message_text,
  notice::{Notice, NoticeTone},
};

pub fn notices(
  ctx: &Ctx,
  detail: &TaskDetail,
  stale: Option<&Message>,
  refused: Option<(RunControl, Message)>,
) -> Vec<Element> {
  let stale = stale.map(|reason| {
    Notice::new(
      "task-read-again-failed",
      NoticeTone::Warning,
      ctx.t("task.notices.staleTitle"),
    )
    .body(message_text(ctx, reason))
    .into()
  });
  let refused = refused.map(|(control, reason)| {
    Notice::new("task-control-refused", NoticeTone::Danger, ctx.t(control.refused_key()))
      .body(message_text(ctx, &reason))
      .into()
  });
  let unreadable = detail.earlier_unreadable.iter().map(|run| {
    Notice::new(
      format!("task-unreadable-{}", run.id),
      NoticeTone::Danger,
      message_text(ctx, &run.title()),
    )
    .icon(Icon::FileWarning)
    .body(message_text(ctx, &run.detail))
    .into()
  });
  stale
    .into_iter()
    .chain(refused)
    .chain(unreadable)
    .chain(blocked(ctx, detail))
    .collect()
}

/// Blocked (`Picker · Not blocked` names the flag): why, and while linked blocking tasks have not
/// finished, that the run neither starts nor goes on until they do.
fn blocked(ctx: &Ctx, detail: &TaskDetail) -> Option<Element> {
  let reason = detail.task.organization().blocked_reason();
  if reason.is_none() && detail.blocked_by.is_empty() {
    return None;
  }
  let body = if detail.blocked_by.is_empty() {
    reason.unwrap_or_default().to_owned()
  } else {
    let tasks = detail.blocked_by.join(&ctx.t("task.listSeparator"));
    let waiting = ctx.t_args("task.notices.blockedBy", [("tasks", tasks.as_str())]);
    match reason {
      Some(reason) => ctx
        .t_args(
          "task.notices.blockedReason",
          [("reason", reason), ("waiting", &waiting)],
        )
        .to_string(),
      None => waiting.to_string(),
    }
  };
  Some(
    Notice::new("task-blocked", NoticeTone::Warning, ctx.t("task.notices.blockedTitle"))
      .icon(Icon::Ban)
      .body(body)
      .into(),
  )
}
