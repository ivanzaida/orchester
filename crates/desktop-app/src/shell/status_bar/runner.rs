//! The runner item while the runner is not online (`05 · Runner starting` `vNIo3`, `06 · Runner offline` `a7yNee`
//! in `Status bar · redesign`; online, the health dot stands for it, `health`): a dot in the state's color and its
//! words, *Runner starting*, *Runner offline*, with the reconnect attempt and its countdown, or why it is offline:
//! *another Orchester app has this project open* when another app runs the project's runner, else how often it was
//! tried and since when (handover §3.3). A click opens the runner log. While it cannot start on its own, *Restart
//! runner* and *Open runner log* follow it; while runs lost their agents, how many follows too. Reconnecting or
//! offline, its card on hover says since when, how often it was tried, when it tries next and how many runs lost
//! their agents (`20 · Runner offline · hover` `nyZ38`).

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
  ui::{
    hover_card::{Fact, FactValue, FactsCard, hover_card},
    icon::Icon,
    pressable::Press,
  },
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
    .child(dot(color.clone()));
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
  let row = row.child(words);
  let mut items = vec![match card(ctx, runner, (color, key)) {
    Some(card) => hover_card(ctx, row, card),
    None => row.into(),
  }];
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

/// The dot's color, the words and the state's name for tools; starting is a warning (`vNIo3`).
fn look(state: &RunnerState) -> (PaletteColor, &'static str, &'static str) {
  match state {
    RunnerState::Starting => (PaletteColor::Warning, "shell.status.runner.starting", "starting"),
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

/// The card of a runner that reconnects or is offline (`ediuF`): its state in its color and since when, how often it
/// was tried, when it tries next, and how many runs lost their agents; none in any other state.
fn card(ctx: &Ctx, runner: &RunnerFacts, (color, key): (PaletteColor, &str)) -> Option<FactsCard> {
  let attempts = match runner.state {
    RunnerState::Reconnecting { attempt, .. } => attempt,
    RunnerState::Failed { attempts, .. } => attempts,
    _ => return None,
  };
  let since = runner.trouble_since.map(|since| {
    ctx
      .t_args("shell.status.health.since", [("time", display::clock_time(since))])
      .to_string()
  });
  let mut facts = Vec::new();
  if attempts > 0 {
    let key = if attempts == 1 {
      "shell.status.health.tried.one"
    } else {
      "shell.status.health.tried.other"
    };
    facts.push(Fact {
      id: "tried",
      label: ctx.t("shell.status.health.triedLabel").to_string(),
      value: FactValue::Words(ctx.t_args(key, [("count", attempts.to_string())]).to_string()),
    });
  }
  if matches!(runner.state, RunnerState::Reconnecting { .. }) {
    facts.push(Fact {
      id: "next",
      label: ctx.t("shell.status.health.nextLabel").to_string(),
      value: FactValue::Clock(Clock::Until(runner.next_attempt)),
    });
  }
  if runner.interrupted > 0 {
    let key = if runner.interrupted == 1 {
      "shell.status.health.lost.one"
    } else {
      "shell.status.health.lost.other"
    };
    facts.push(Fact {
      id: "lost",
      label: ctx.t("shell.status.health.lostLabel").to_string(),
      value: FactValue::Words(ctx.t_args(key, [("count", runner.interrupted.to_string())]).to_string()),
    });
  }
  Some(FactsCard {
    id: "status-runner-card".into(),
    dot: (color.clone(), color),
    state: Some((ctx.t(key).to_string(), since)),
    facts,
    note: None,
    from_start: true,
    gives_way: Some(give_way::RUNNER_WORDS),
  })
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
