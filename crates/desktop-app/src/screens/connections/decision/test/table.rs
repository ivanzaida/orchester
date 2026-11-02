//! The four columns of gnuuu; widths from QYs88, HzIMC and C1Jv7.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::review::{DecisionTestReport, ReviewerChoice, default_decision_threshold};

use crate::theme::{ConnectionsText, ExtraColor, ProjectSettingsSpacing, ProjectSettingsText};

const WIDTHS: [f32; 3] = [90.0, 170.0, 150.0];

pub(super) fn render(ctx: &Ctx, report: Option<&DecisionTestReport>, reviewer: Option<&ReviewerChoice>) -> Element {
  let header = header(ctx);
  let rows = report.into_iter().flat_map(|report| {
    report.thresholds.iter().map(move |figures| {
      let selected = reviewer.is_some_and(|r| {
        r.model == report.model && r.threshold.unwrap_or_else(default_decision_threshold) == figures.threshold
      });
      let approved = ctx.t_args(
        "decision_connection.testApprovedCount",
        [
          ("count", figures.approves.to_string()),
          (
            "percent",
            format!(
              "{:.1}",
              if report.tested == 0 {
                0.0
              } else {
                figures.approves as f64 * 100.0 / report.tested as f64
              }
            ),
          ),
        ],
      );
      let earlier = earlier_count(
        ctx,
        figures.previously_asked_now_approved,
        report.previously_asked,
        selected,
      );
      let row = line()
        .id(format!("decision-test-threshold-{:.2}", figures.threshold.value()))
        .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(cell(
          0,
          Text::new(&format!("{:.2}", figures.threshold.value()))
            .variant(ProjectSettingsText::MonoValue.role())
            .color(PaletteColor::TextPrimary),
        ))
        .child(cell(1, Text::new(&approved).variant(ConnectionsText::TestCell.role())))
        .child(cell(
          2,
          Text::new(&figures.asks_person.to_string()).variant(ConnectionsText::TestCell.role()),
        ))
        .child(cell(
          3,
          Text::new(&earlier)
            .variant(ConnectionsText::TestCell.role())
            .color(if selected {
              ExtraColor::AccentText.role()
            } else {
              PaletteColor::TextMuted
            }),
        ));
      if selected {
        row
          .background(PaletteColor::AccentMuted)
          .describe("reviewer-threshold", "true")
      } else {
        row
      }
    })
  });
  Column::new()
    .id("decision-test-table")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(header)
    .with_children(rows)
    .into()
}

fn earlier_count(ctx: &Ctx, count: u64, total: Option<u64>, selected: bool) -> std::sync::Arc<str> {
  ctx.t_args(
    if selected {
      "decision_connection.testEarlierUsed"
    } else {
      "decision_connection.testEarlierCount"
    },
    [
      ("count", count.to_string()),
      (
        "total",
        total.map_or_else(
          || ctx.t("decision_connection.testUnknown").to_string(),
          |n| n.to_string(),
        ),
      ),
    ],
  )
}

fn header(ctx: &Ctx) -> Row {
  let headings = [
    "decision_connection.testThreshold",
    "decision_connection.testApproves",
    "decision_connection.testComes",
    "decision_connection.testEarlier",
  ];
  line().background(PaletteColor::SurfacePanel).with_children(
    headings
      .into_iter()
      .enumerate()
      .map(|(i, key)| cell(i, Text::new(&ctx.t(key)).variant(ConnectionsText::CompactLine.role()))),
  )
}
fn line() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .padding(Padding::symmetric(ProjectSettingsSpacing::RowX.role(), SpacingSize::Sm))
    .align_items(Alignment::Center)
}

fn cell(index: usize, text: Text) -> Element {
  let cell = Column::new().child(text.width(Dimension::Pct(100.0)));
  if let Some(width) = WIDTHS.get(index) {
    cell.width(Dimension::Px(*width)).into()
  } else {
    cell.flex(1.0).min_width(Dimension::Px(0.0)).into()
  }
}
