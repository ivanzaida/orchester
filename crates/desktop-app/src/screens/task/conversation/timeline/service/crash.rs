//! A service that crashed after it was ready (`06b · Conversation · service crashed` `FwHrv`, `Card · cloud-api
//! crashed` `tz1d8`), from the runner's `service_crashed` note: in a `danger` stroke, a `circle-x` before *cloud-api
//! crashed* and how it ended, when, and how long after it started; what Orchester stopped with it; its last lines with
//! Orchester's own line of how it ended last; then Restart, *Open log* and Edit. The note names no root cause, so the
//! title names only the service, and its lines are the crashed service's own.

use std::time::Duration;

use lurq::{app::ctx::Ctx, app::theme::PaletteColor, node::Element};
use orchester_core::{service::ServiceCrash, time::Timestamp};
use orchester_workspace::display;

use super::{super::Timeline, actions::LogPlace};
use crate::ui::{
  icon::Icon,
  service_card::{Meta, TailLine, body, frame, head, log_tail},
};

pub(super) fn card(ctx: &mut Ctx, timeline: &Timeline, id: &str, (crash, at): (&ServiceCrash, Timestamp)) -> Element {
  let service = crash.service.as_str();
  let seconds = format!("{:.1}", Duration::from_millis(crash.uptime_ms).as_secs_f64());
  let time = display::clock_seconds(at);
  let (meta, ended) = match crash.exit_code {
    Some(code) => (
      ctx.t_args(
        "service_cards.crashed.meta",
        [("code", code.to_string()), ("time", time), ("seconds", seconds)],
      ),
      ctx.t_args(
        "service_cards.crashed.exited",
        [("service", service.to_owned()), ("code", code.to_string())],
      ),
    ),
    None => (
      ctx.t_args("service_cards.crashed.metaSignal", [("time", time), ("seconds", seconds)]),
      ctx.t_args("service_cards.crashed.endedBySignal", [("service", service.to_owned())]),
    ),
  };
  let said = if crash.stopped.is_empty() {
    ctx.t_args("service_cards.crashed.alone", [("service", service.to_owned())])
  } else {
    ctx.t_args(
      "service_cards.crashed.stopped",
      [("service", service.to_owned()), ("services", super::listed(ctx, &crash.stopped))],
    )
  };
  let lines: Vec<TailLine> = crash
    .lines
    .iter()
    .map(|row| TailLine::of_row(service, row))
    .chain([TailLine {
      source: ctx.t("service_cards.orchester"),
      text: ended,
      danger: true,
    }])
    .collect();
  let title = ctx.t_args("service_cards.crashed.title", [("service", service.to_owned())]);
  let labels = (
    ctx.t_args("service_cards.actions.restartService", [("service", service.to_owned())]),
    ctx.t_args("service_cards.actions.editService", [("service", service.to_owned())]),
  );
  let actions = super::actions(ctx, timeline, (id, service), (labels, LogPlace::Second));
  frame(id, PaletteColor::Danger)
    .describe("service", service.to_owned())
    .child(head(id, (Icon::CircleX, PaletteColor::Danger), &title, (&meta, Meta::AfterTitle)))
    .child(body(&format!("{id}-body"), &said))
    .child(log_tail(&format!("{id}-lines"), &lines))
    .with_children(actions)
    .into()
}
