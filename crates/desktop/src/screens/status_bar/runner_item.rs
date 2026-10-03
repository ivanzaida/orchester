//! The runner item (`N8rki`): the open project's runner as icon and label, with the reconnect
//! attempt (E) or how long it has been offline (G). It always opens the runner log.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  layout::layout_kind::ShrinkLimit,
  node::Element,
};

use super::{
  ProjectBar,
  hint::{HintAnchor, Hints},
  item::{clickable, dropping_label, give_way, glyph, item, value},
  open::Opener,
  timed::{Retry, RetryProps},
};
use crate::{
  activity::bar::RunnerBadge,
  display, messages,
  runner::state::{RECONNECT_LIMIT, RunnerState},
  ui::{icon::Icon, message_text},
};

pub fn runner_item(ctx: &mut Ctx, project: &ProjectBar, opener: &Opener, hints: &Hints) -> Element {
  let state = project.runner.state_for(&project.repo_root);
  let badge = RunnerBadge::of(&state);
  let (icon, key) = look(badge);
  let color = if matches!(badge, RunnerBadge::Reconnecting | RunnerBadge::Offline) {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  // Its words are the last of the left group to give way ([`give_way`]); the icon keeps the state.
  let mut row = item("status-runner")
    .describe("state", state_name(badge))
    .flex_shrink(1.0)
    .shrink_order(give_way::RUNNER_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(icon, color.clone()))
    .child(dropping_label(&ctx.t(key), color));
  match &state {
    RunnerState::Reconnecting { attempt, .. } => {
      row = row.child(ctx.mount_keyed::<Retry>(
        "status-runner-retry",
        RetryProps {
          attempt: *attempt,
          limit: RECONNECT_LIMIT,
          next: project.runner.until(),
        },
      ));
    }
    RunnerState::Failed { attempts, cause } => {
      row = row.child(value(&offline_detail(ctx, *attempts, project), PaletteColor::TextMuted));
      row = hints.attach(
        row,
        HintAnchor::Runner,
        message_text(ctx, &messages::runner_failure(cause)),
      );
    }
    _ => {}
  }
  let opener = opener.clone();
  clickable(row, move || opener.open_log()).into()
}

/// Icon and label of each badge (states A, B, E, G of `keJ9Z`).
fn look(badge: RunnerBadge) -> (Icon, &'static str) {
  match badge {
    RunnerBadge::Starting => (Icon::Loader, "statusBar.runner.starting"),
    RunnerBadge::Online => (Icon::Plug, "statusBar.runner.online"),
    RunnerBadge::Reconnecting => (Icon::RefreshCw, "statusBar.runner.reconnecting"),
    RunnerBadge::Stopping => (Icon::Loader, "statusBar.runner.stopping"),
    RunnerBadge::Stopped => (Icon::Unplug, "statusBar.runner.stopped"),
    RunnerBadge::Offline => (Icon::Unplug, "statusBar.runner.offline"),
  }
}

/// "5 attempts since 14:02", or that it never started.
fn offline_detail(ctx: &Ctx, attempts: u32, project: &ProjectBar) -> Arc<str> {
  let since = project.runner.trouble_since().map(display::clock_time);
  let count = attempts.to_string();
  match (attempts, since) {
    (0, _) => ctx.t("statusBar.runner.notStarted"),
    (1, Some(time)) => ctx.t_args(
      "statusBar.runner.attemptsSince.one",
      [("attempts", count.as_str()), ("time", time.as_str())],
    ),
    (_, Some(time)) => ctx.t_args(
      "statusBar.runner.attemptsSince.other",
      [("attempts", count.as_str()), ("time", time.as_str())],
    ),
    (1, None) => ctx.t_args("statusBar.runner.attempts.one", [("attempts", count.as_str())]),
    (_, None) => ctx.t_args("statusBar.runner.attempts.other", [("attempts", count.as_str())]),
  }
}

/// For tooling (`lurq_read_tree`): the runner state without reading the copy.
fn state_name(badge: RunnerBadge) -> &'static str {
  match badge {
    RunnerBadge::Starting => "starting",
    RunnerBadge::Online => "online",
    RunnerBadge::Reconnecting => "reconnecting",
    RunnerBadge::Stopping => "stopping",
    RunnerBadge::Stopped => "stopped",
    RunnerBadge::Offline => "offline",
  }
}
