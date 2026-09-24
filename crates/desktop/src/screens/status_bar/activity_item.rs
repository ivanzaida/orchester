//! The activity slot (`IRKYz` with its actions): the runner problem's actions (G), the newest
//! unresolved failure (H), running operations (C, D) or the latest event (A, F).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};

use super::{
  hint::{HintAnchor, Hints},
  item::{action, clickable, dismiss, glyph, item, separator, truncated_label, value},
  open::Opener,
  timed::{Elapsed, ElapsedProps},
};
use crate::{
  activity::{
    ActivityLog, Event, Operation, OperationId, Outcome, Slot, Target,
    bar::{ActivitySlot, RunnerBadge},
  },
  display,
  ui::{icon::Icon, message_text},
};

/// The slot's elements; `after_runner` puts a separator after the runner item.
pub fn activity_slot(
  ctx: &mut Ctx,
  slot: &ActivitySlot<'_>,
  log: &ActivityLog,
  opener: &Opener,
  hints: &Hints,
  after_runner: bool,
) -> Vec<Element> {
  let items = match slot {
    // The design names interrupted runs here (E, G); runs do not exist yet, so only G's actions
    // are shown.
    ActivitySlot::Hidden | ActivitySlot::RunnerProblem(RunnerBadge::Reconnecting) => Vec::new(),
    ActivitySlot::RunnerProblem(_) => runner_actions(ctx, opener),
    ActivitySlot::Activity(Slot::Empty) => Vec::new(),
    ActivitySlot::Activity(Slot::Failure(event)) => failure(ctx, event, opener, hints),
    ActivitySlot::Activity(Slot::Running { newest, count }) => {
      vec![running(ctx, newest, *count, log, opener, hints)]
    }
    ActivitySlot::Activity(Slot::Latest(event)) => vec![latest(ctx, event, opener, hints)],
  };
  if items.is_empty() || !after_runner {
    return items;
  }
  std::iter::once(separator()).chain(items).collect()
}

/// Whether the slot shows the activity item, which anchors the popover.
pub fn shows_activity(slot: &ActivitySlot<'_>) -> bool {
  matches!(slot, ActivitySlot::Activity(slot) if *slot != Slot::Empty)
}

/// "Restart runner" and "Open runner log" (G).
fn runner_actions(ctx: &Ctx, opener: &Opener) -> Vec<Element> {
  let (restart, log) = (opener.clone(), opener.clone());
  vec![
    action(
      "status-restart-runner",
      &ctx.t("runner.connection.restart"),
      move || {
        restart.restart();
      },
    ),
    action("status-open-log", &ctx.t("runner.connection.openLog"), move || {
      log.open_log();
    }),
  ]
}

/// H: the failure on a raised fill, the action that opens its object, and dismiss.
fn failure(ctx: &Ctx, event: &Event, opener: &Opener, hints: &Hints) -> Vec<Element> {
  let text = event_text(ctx, event);
  let row = item("status-activity")
    .describe("state", "failure")
    .background(PaletteColor::SurfaceRaised)
    .child(glyph(Icon::AlertCircle, PaletteColor::TextPrimary))
    .child(truncated_label(&text, PaletteColor::TextPrimary))
    .child(value(&display::clock_time(event.at()), PaletteColor::TextMuted));
  let row = hints.attach(row, HintAnchor::Activity, text);
  let mut items = vec![open_event(row, event, opener)];
  if opener.can_open(event.target()) {
    let (target, id, open) = (event.target().clone(), event.id(), opener.clone());
    items.push(action(
      "status-activity-open",
      &ctx.t(super::popover::open_key(&target)),
      move || open.open(&target, Some(id)),
    ));
  }
  let (id, dismissing) = (event.id(), opener.clone());
  items.push(dismiss("status-activity-dismiss", move || dismissing.dismiss(id)));
  items
}

/// C: the operation and its elapsed time; D: the count and the newest, opening the popover.
fn running(
  ctx: &mut Ctx,
  newest: &Operation,
  count: usize,
  log: &ActivityLog,
  opener: &Opener,
  hints: &Hints,
) -> Element {
  let title = message_text(ctx, newest.title());
  if count > 1 {
    let count = count.to_string();
    let text = ctx.t_args(
      "statusBar.activity.several",
      [("count", count.as_str()), ("title", &*title)],
    );
    return several(&text, log, opener, hints);
  }
  let text = ctx.t_args("statusBar.activity.running", [("title", &*title)]);
  let row = item("status-activity")
    .describe("state", "running")
    .child(glyph(Icon::Loader, PaletteColor::TextSecondary))
    .child(truncated_label(&text, PaletteColor::TextSecondary))
    .child(ctx.mount_keyed::<Elapsed>(
      &format!("status-elapsed-{:?}", newest.id()),
      ElapsedProps {
        since: newest.started(),
      },
    ));
  let row = hints.attach(row, HintAnchor::Activity, text);
  open_or_popover(row, newest.target().clone(), None, opener)
}

/// D: the count and the newest operation, opening the popover.
fn several(text: &Arc<str>, log: &ActivityLog, opener: &Opener, hints: &Hints) -> Element {
  let row = item("status-activity")
    .describe("state", "several")
    .describe("running", log.running().count().to_string())
    .child(glyph(Icon::Loader, PaletteColor::TextPrimary))
    .child(truncated_label(text, PaletteColor::TextPrimary))
    .child(glyph(Icon::ChevronUp, PaletteColor::TextPrimary));
  let row = hints.attach(row, HintAnchor::Activity, text.clone());
  popover_toggle(row, opener)
}

/// A, F: the latest event with its time.
fn latest(ctx: &Ctx, event: &Event, opener: &Opener, hints: &Hints) -> Element {
  let text = event_text(ctx, event);
  let icon = match event.outcome() {
    Outcome::Finished => Icon::Check,
    Outcome::Failed { .. } => Icon::AlertCircle,
  };
  let row = item("status-activity")
    .describe("state", "latest")
    .child(glyph(icon, PaletteColor::TextSecondary))
    .child(truncated_label(&text, PaletteColor::TextSecondary))
    .child(value(&display::clock_time(event.at()), PaletteColor::TextMuted));
  let row = hints.attach(row, HintAnchor::Activity, text);
  open_event(row, event, opener)
}

fn open_event(row: Row, event: &Event, opener: &Opener) -> Element {
  let failure = event.is_unresolved().then(|| event.id());
  open_or_popover(row, event.target().clone(), failure, opener)
}

/// Opens the object the item names; with nothing to open, the popover with the history.
fn open_or_popover(row: Row, target: Target, failure: Option<OperationId>, opener: &Opener) -> Element {
  if opener.can_open(&target) {
    let opener = opener.clone();
    clickable(row, move || opener.open(&target, failure)).into()
  } else {
    popover_toggle(row, opener)
  }
}

/// An item that toggles the popover; it keeps the `$hover` fill while the popover is open.
fn popover_toggle(row: Row, opener: &Opener) -> Element {
  let row = if opener.popover_open() {
    row.background(PaletteColor::AccentMuted)
  } else {
    row
  };
  let toggle = opener.clone();
  clickable(row, move || toggle.toggle_popover()).into()
}

/// An event's text: its title, and the reason of a failure.
pub fn event_text(ctx: &Ctx, event: &Event) -> Arc<str> {
  let title = message_text(ctx, event.title());
  match event.outcome() {
    Outcome::Finished => title,
    Outcome::Failed { reason, .. } => {
      let reason = message_text(ctx, reason);
      ctx.t_args("statusBar.activity.failure", [("title", &*title), ("reason", &*reason)])
    }
  }
}
