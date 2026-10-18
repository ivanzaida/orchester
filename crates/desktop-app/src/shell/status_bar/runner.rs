//! The runner item (`Status · runner` `hiR3M`): a dot in the state's color and its words, *Runner online*
//! (`hiR3M`), *Runner starting* (`F6u81K` in `CHfHU`), *Runner offline* (`Dn5SA` in `KGpMB`), with the
//! reconnect attempt and its countdown, or why it is offline: *another Orchester app has this project
//! open* when another app runs the project's runner, else how often it was tried and since when
//! (handover §3.3). A click opens the runner log. While it cannot start on its own, *Restart runner* and
//! *Open runner log* follow it; while runs lost their agents, how many follows too.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, padding::Padding},
};
use orchester_workspace::{
  display,
  runner::state::{RECONNECT_LIMIT, RunnerFailure, RunnerState},
};

use super::{
  facts::{BarAction, Dispatch, RunnerFacts},
  item::{action, cut, dot, dropping, give_way, glyph, item, pressed, time_part},
};
use crate::{
  shell::clock::{Clock, clock},
  theme::ExtraSpacing,
  ui::{icon::Icon, pressable::Press},
};

/// The runner item and, while it cannot start on its own, its actions.
pub fn runner(ctx: &mut Ctx, runner: &RunnerFacts, dispatch: &Dispatch) -> Vec<Element> {
  let (color, key, state) = look(&runner.state);
  let open = dispatch.clone();
  // The words and what follows them (*· another Orchester app…*, the countdown) are one space apart.
  let mut words = Row::new()
    .flex_shrink(1.0)
    .shrink_limit(ShrinkLimit::Content)
    .spacing(ExtraSpacing::Fine.role())
    .align_items(Alignment::Center)
    .child(dropping(&ctx.t(key), PaletteColor::TextMuted));
  let mut row = pressed("status-runner", Press::run(move || open(BarAction::OpenRunnerLog)))
    .describe("state", state)
    .flex_shrink(1.0)
    .shrink_order(give_way::RUNNER_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .child(dot(color));
  let mut retry = None;
  match &runner.state {
    RunnerState::Reconnecting { attempt, .. } => {
      let next = Clock::Retry {
        attempt: *attempt,
        limit: RECONNECT_LIMIT,
        next: runner.next_attempt,
      };
      // The countdown is one of the bar's times: it drops out with them.
      row = row.padding(Padding::horizontal(SpacingSize::Sm).right(ExtraSpacing::Fine.role()));
      retry = Some(time_part(
        "status-runner-retry",
        clock(ctx, "status-runner-retry-text", next),
      ));
    }
    RunnerState::Failed { cause, attempts } => {
      let detail = offline_detail(ctx, cause, *attempts, runner);
      row = row.describe("cause", cause_name(cause));
      words = words.child(dropping(&detail, PaletteColor::TextMuted).id("status-runner-detail"));
    }
    _ => {}
  }
  let mut items = vec![row.child(words).into()];
  items.extend(retry);
  items.extend(interrupted(ctx, runner));
  if runner.state.can_restart() {
    let (restart, log) = (dispatch.clone(), dispatch.clone());
    items.push(action(
      "status-restart-runner",
      &ctx.t("shell.status.runner.restart"),
      Press::run(move || restart(BarAction::RestartRunner)),
    ));
    items.push(action(
      "status-open-runner-log",
      &ctx.t("shell.status.runner.openLog"),
      Press::run(move || log(BarAction::OpenRunnerLog)),
    ));
  }
  items
}

/// The dot's color, the words and the state's name for tools.
fn look(state: &RunnerState) -> (PaletteColor, &'static str, &'static str) {
  match state {
    RunnerState::Starting => (PaletteColor::TextMuted, "shell.status.runner.starting", "starting"),
    RunnerState::Online => (PaletteColor::Success, "shell.status.runner.online", "online"),
    RunnerState::Reconnecting { .. } => (
      PaletteColor::Warning,
      "shell.status.runner.reconnecting",
      "reconnecting",
    ),
    RunnerState::Stopping => (PaletteColor::TextMuted, "shell.status.runner.stopping", "stopping"),
    RunnerState::Stopped => (PaletteColor::TextMuted, "shell.status.runner.stopped", "stopped"),
    RunnerState::Failed { .. } => (PaletteColor::Danger, "shell.status.runner.offline", "offline"),
  }
}

/// Why the runner is offline in a few words: another app has the project, its program is missing, or how
/// often it was tried and since when.
fn offline_detail(ctx: &Ctx, cause: &RunnerFailure, attempts: u32, runner: &RunnerFacts) -> String {
  let key = match (cause, attempts, runner.trouble_since) {
    (RunnerFailure::AlreadyServed, ..) => "shell.status.runner.alreadyServed",
    (RunnerFailure::MissingExecutable { .. }, ..) => "shell.status.runner.missing",
    (_, 0, _) => "shell.status.runner.notStarted",
    (_, 1, Some(_)) => "shell.status.runner.attemptsSince.one",
    (_, _, Some(_)) => "shell.status.runner.attemptsSince.other",
    (_, 1, None) => "shell.status.runner.attempts.one",
    (_, _, None) => "shell.status.runner.attempts.other",
  };
  let time = runner.trouble_since.map(display::clock_time).unwrap_or_default();
  ctx
    .t_args(key, [("attempts", attempts.to_string()), ("time", time)])
    .to_string()
}

const fn cause_name(cause: &RunnerFailure) -> &'static str {
  match cause {
    RunnerFailure::MissingExecutable { .. } => "missing_executable",
    RunnerFailure::AlreadyServed => "already_served",
    RunnerFailure::StartFailed => "start_failed",
    RunnerFailure::Lost => "lost",
  }
}

/// *2 runs interrupted · they continue from their last checkpoint* while the runner reconnects, *… ·
/// nothing runs until the runner starts* once it gave up; nothing when no agent was at work.
fn interrupted(ctx: &mut Ctx, runner: &RunnerFacts) -> Option<Element> {
  let reconnecting = match runner.state {
    RunnerState::Reconnecting { .. } => true,
    RunnerState::Failed { .. } => false,
    _ => return None,
  };
  let key = match (reconnecting, runner.interrupted) {
    (_, 0) => return None,
    (true, 1) => "shell.status.interrupted.reconnecting.one",
    (true, _) => "shell.status.interrupted.reconnecting.other",
    (false, 1) => "shell.status.interrupted.offline.one",
    (false, _) => "shell.status.interrupted.offline.other",
  };
  let text = ctx.t_args(key, [("count", runner.interrupted.to_string())]);
  Some(
    item("status-interrupted")
      .flex_shrink(1.0)
      .shrink_order(give_way::ACTIVITY)
      .shrink_limit(ShrinkLimit::Content)
      .child(glyph(Icon::CirclePause, PaletteColor::Warning))
      .child(cut(ctx, "status-interrupted-text", &text, PaletteColor::TextSecondary))
      .into(),
  )
}
