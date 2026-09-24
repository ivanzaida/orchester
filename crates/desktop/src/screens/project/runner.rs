//! The open project's runner at the top of the overview: the notice that it could not be
//! started or restarted (02.H `EFY7I`), which blocks all work and needs a decision. Starting,
//! reconnecting and reconnected are ambient and shown only in the status bar (`keJ9Z`).

use lurq::{
  app::ctx::Ctx,
  node::{Element, dimension::Dimension},
};

use crate::{
  messages,
  runner::{Runners, state::RunnerState},
  ui::{button::ActionButton, icon::Icon, message_text, notice::Notice},
};

/// Notice action height (`atylT`, `Yfte7`).
const NOTICE_ACTION_HEIGHT: f32 = 32.0;

/// The notice, while the runner has given up.
pub fn connection_notice(ctx: &Ctx, state: &RunnerState, runners: &Runners) -> Option<Element> {
  let RunnerState::Failed { cause, attempts } = state else {
    return None;
  };
  let (title, detail) = if *attempts == 0 {
    (
      ctx.t("runner.connection.notStartedTitle"),
      message_text(ctx, &messages::runner_failure(cause)),
    )
  } else {
    let attempts = attempts.to_string();
    (
      ctx.t("runner.connection.notRestartedTitle"),
      ctx.t_args(
        "runner.connection.notRestartedDetail",
        [("attempts", attempts.as_str())],
      ),
    )
  };
  let notice = Notice::new(Icon::Unplug, title, detail)
    .action(open_log(ctx, runners))
    .action(restart(ctx, runners));
  Some(notice.emphasized().id("runner-notice").into())
}

/// "Open runner log" (`atylT`).
fn open_log(ctx: &Ctx, runners: &Runners) -> Element {
  let runners = runners.clone();
  ActionButton::new(ctx.t("runner.connection.openLog"))
    .icon(Icon::FileText)
    .id("runner-open-log")
    .height(Dimension::Px(NOTICE_ACTION_HEIGHT))
    .on_click(move || runners.open_log())
    .render(ctx)
}

/// "Restart runner" (`Yfte7`).
fn restart(ctx: &Ctx, runners: &Runners) -> Element {
  let runners = runners.clone();
  ActionButton::new(ctx.t("runner.connection.restart"))
    .primary()
    .icon(Icon::RefreshCw)
    .id("runner-restart")
    .height(Dimension::Px(NOTICE_ACTION_HEIGHT))
    .on_click(move || runners.restart())
    .render(ctx)
}
