//! The runner item (`N8rki`): the open project's runner as icon and label, with the reconnect
//! attempt (E) or how long it has been offline (G). It always opens the runner log.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};

use super::{
  ProjectBar,
  hint::{HintAnchor, Hints},
  item::{clickable, glyph, item, label, value},
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
  let mut row = item("status-runner")
    .describe("state", state_name(badge))
    .child(glyph(icon, color.clone()))
    .child(label(&ctx.t(key), color));
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
  match (attempts, since) {
    (0, _) => ctx.t("statusBar.runner.notStarted"),
    (attempts, Some(time)) => {
      let attempts = attempts.to_string();
      ctx.t_args(
        "statusBar.runner.attemptsSince",
        [("attempts", attempts.as_str()), ("time", time.as_str())],
      )
    }
    (attempts, None) => {
      let attempts = attempts.to_string();
      ctx.t_args("statusBar.runner.attempts", [("attempts", attempts.as_str())])
    }
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
