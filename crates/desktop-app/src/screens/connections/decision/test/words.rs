//! Complete translated sentences; absent usage and cost are never made up.
use lurq::{app::ctx::Ctx, components::Text, node::dimension::Dimension};
use orchester_core::{
  connection::DecisionConnection,
  run::review::{DecisionTestReport, DecisionTestStatus},
};
use orchester_workspace::display;

use crate::theme::ConnectionsText;

pub(super) fn lead(ctx: &Ctx) -> Text {
  Text::new(&ctx.t("decision_connection.testLead"))
    .variant(ConnectionsText::TestLead.role())
    .width(Dimension::Pct(100.0))
}

pub(super) fn sent(ctx: &Ctx, connection: &DecisionConnection) -> Text {
  Text::new(&ctx.t_args("decision_connection.testSent", [("host", connection.base_url().host())]))
    .variant(ConnectionsText::CompactLine.role())
    .width(Dimension::Pct(100.0))
}

pub(super) fn meta(ctx: &Ctx, report: Option<&DecisionTestReport>, running: bool) -> std::sync::Arc<str> {
  let Some(report) = report else {
    return ctx.t(if running {
      "decision_connection.testStarting"
    } else {
      "decision_connection.testEmpty"
    });
  };
  if running {
    return ctx.t_args(
      "decision_connection.testProgress",
      [
        ("tested", report.tested.to_string()),
        ("requests", report.requests.to_string()),
        ("flight", report.in_flight.to_string()),
      ],
    );
  }
  let status = ctx.t(match report.status {
    DecisionTestStatus::Completed => "decision_connection.testCompleted",
    DecisionTestStatus::Stopped => "decision_connection.testStopped",
    DecisionTestStatus::Interrupted => "decision_connection.testInterrupted",
    DecisionTestStatus::Running => "decision_connection.testStarting",
  });
  let (month, day) = display::month_day(report.started_at);
  let date = ctx.t_args(
    "decision_connection.testDate",
    [
      ("day", day.to_string()),
      ("month", ctx.t_ns("workspace", display::MONTH_KEYS[month]).to_string()),
      ("time", display::clock(report.started_at)),
    ],
  );
  ctx.t_args(
    "decision_connection.testMeta",
    [
      ("status", status.to_string()),
      ("time", date.to_string()),
      ("model", report.model.as_str().to_owned()),
      ("tested", report.tested.to_string()),
      ("requests", report.requests.to_string()),
      (
        "median",
        report.median_ms.map_or_else(
          || ctx.t("decision_connection.testUnknown").to_string(),
          |n| n.to_string(),
        ),
      ),
      ("tokens", report.input_tokens.to_string()),
    ],
  )
}

pub(super) fn caveat(ctx: &Ctx, report: &DecisionTestReport) -> Option<Text> {
  (report.unknown_usage > 0 || report.failed > 0 || report.too_long > 0 || !report.omitted_fields.is_empty()).then(
    || {
      Text::new(&ctx.t_args(
        "decision_connection.testCaveat",
        [
          ("unknown", report.unknown_usage.to_string()),
          ("failed", report.failed.to_string()),
          ("skipped", report.too_long.to_string()),
          ("omitted", report.omitted_fields.len().to_string()),
        ],
      ))
      .id("decision-test-caveat")
      .variant(ConnectionsText::CompactLine.role())
      .width(Dimension::Pct(100.0))
    },
  )
}
