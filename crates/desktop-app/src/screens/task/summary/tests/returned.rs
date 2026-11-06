//! The run's last event while the step a Test step's failed tests went back to works on them (`Last event` `cy82c`
//! in `qN2FK`): "04 Test returned the 2 failed tests to 03 Implement · attempt 3", in one line after a
//! `corner-down-left`.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::run::AttemptReason;
use orchester_workspace::tasks::detail::RunView;

use super::step_label;
use crate::{
  screens::task::summary::notice::Said,
  ui::{icon::Icon, notice::NoticeTone},
};

/// What the last event says while `run`'s current attempt works on the failed tests its Test step returned; `None`
/// at any other time.
pub fn said(ctx: &Ctx, run: &RunView) -> Option<Said> {
  let attempt = run.run.current_attempt()?;
  let AttemptReason::Retry { failed_step, return_to } = attempt.reason() else {
    return None;
  };
  let tests = run.tests.as_ref()?;
  let record = tests.record.as_ref().ok()?;
  let at_work = run.working_steps().any(|step| step.key == *return_to);
  if tests.step != *failed_step || tests.returned_to.as_ref() != Some(return_to) || !at_work {
    return None;
  }
  let count = record.failed().count();
  let key = if count == 1 {
    "summary.notice.testsReturnedTitle.one"
  } else {
    "summary.notice.testsReturnedTitle.other"
  };
  let title = ctx.t_args(
    key,
    [
      ("step", step_label(run, failed_step.as_str())),
      ("count", count.to_string()),
      ("to", step_label(run, return_to.as_str())),
      ("attempt", attempt.number().get().to_string()),
    ],
  );
  Some(Said {
    tone: NoticeTone::Info,
    icon: Icon::CornerDownLeft,
    title,
    body: Arc::from(""),
  })
}
