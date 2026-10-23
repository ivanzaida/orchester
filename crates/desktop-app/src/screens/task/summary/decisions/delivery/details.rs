//! *Delivery* under *Run details* of a run whose pipeline ends in a Deliver step (`Section · Delivery` in `ulEuh`,
//! `lvupO`): a `border` line, the title, and how the run is delivered: its mode (and who approved it when), its merge
//! method, the integrator, where it stands in the queue and how many times it synced again.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::merge_queue::{MAX_RESYNCS, QueueState};
use orchester_workspace::{display::clock, merge_queue::TaskDelivery};

use crate::{
  screens::merge_queue::words,
  theme::{ExtraSpacing, TaskText},
  ui::detail_row::{DetailValue, detail_row},
};

/// The divider's thickness.
const DIVIDER: f32 = 1.0;

/// The section's divider, title and rows, when the run is delivered by the queue.
pub fn rows(ctx: &Ctx, queue: Option<&TaskDelivery>) -> Vec<Element> {
  let Some(queue) = queue else {
    return Vec::new();
  };
  let entry = queue.delivery.as_ref().map(|delivery| &delivery.entry);
  let mode = words::mode(ctx, queue.deliver.mode());
  let mode = match entry.and_then(|entry| entry.approved_at) {
    Some(at) => ctx.t_args(
      "delivery.task.details.approvedAt",
      [("mode", mode.to_string()), ("time", clock(at))],
    ),
    None => mode,
  };
  let method = match queue.deliver.method() {
    orchester_core::resource::MergeMethod::MergeCommit => ctx.t("delivery.step.mergeCommit"),
    orchester_core::resource::MergeMethod::Squash => ctx.t("delivery.step.squash"),
  };
  let integrator: Arc<str> = queue
    .integrator
    .as_deref()
    .map_or_else(|| ctx.t("delivery.task.details.noIntegrator"), Arc::from);
  let place = queue.delivery.as_ref().map(|delivery| queue_value(ctx, delivery));
  let resyncs = entry.map(|entry| {
    ctx.t_args(
      "delivery.task.details.resyncsValue",
      [("count", entry.resyncs.to_string()), ("max", MAX_RESYNCS.to_string())],
    )
  });
  let row = |id: &str, key: &str, value: &str| -> Element {
    detail_row(
      &format!("task-delivery-detail-{id}"),
      &ctx.t(key),
      value,
      DetailValue::Text,
    )
    .into()
  };
  let mut rows = vec![
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(DIVIDER))
      .background(PaletteColor::Border)
      .into(),
    Column::new()
      .padding(
        Padding::new()
          .top(ExtraSpacing::Loose.role())
          .bottom(ExtraSpacing::Hair.role()),
      )
      .child(
        Text::new(&ctx.t("delivery.task.details.title"))
          .id("task-delivery-details-title")
          .variant(TaskText::SectionTitle.role()),
      )
      .into(),
    row("mode", "delivery.task.details.mode", &mode),
    row("method", "delivery.task.details.method", &method),
    row("integrator", "delivery.task.details.integrator", &integrator),
  ];
  rows.extend(place.map(|place| row("queue", "delivery.task.details.queue", &place)));
  rows.extend(resyncs.map(|resyncs| row("resyncs", "delivery.task.details.resyncs", &resyncs)));
  rows
}

/// Where the run stands in the queue: its place, or its state and since when.
fn queue_value(ctx: &Ctx, delivery: &orchester_workspace::merge_queue::RunDelivery) -> Arc<str> {
  let entry = &delivery.entry;
  let since = clock(entry.since);
  match (&entry.state, delivery.place) {
    (state, _) if state.delivering() => ctx.t_args("delivery.task.details.delivering", [("time", since)]),
    (QueueState::Queued, Some(place)) => ctx.t_args(
      "delivery.task.details.waiting",
      [("place", words::place(ctx, place).to_string()), ("time", since)],
    ),
    (QueueState::WaitingForApproval, _) => ctx.t_args("delivery.task.details.approval", [("time", since)]),
    (QueueState::Blocked(_), _) => ctx.t_args("delivery.task.details.blocked", [("time", since)]),
    (QueueState::Left(left), _) if left.failed() => ctx.t_args("delivery.task.details.failed", [("time", since)]),
    (QueueState::Left(_), _) => ctx.t_args("delivery.task.details.left", [("time", since)]),
    (QueueState::Merged(_), _) => ctx.t_args("delivery.task.details.merged", [("time", since)]),
    (QueueState::Removed, _) => ctx.t_args("delivery.task.details.removed", [("time", since)]),
    _ => ctx.t_args("delivery.task.details.joined", [("time", since)]),
  }
}
