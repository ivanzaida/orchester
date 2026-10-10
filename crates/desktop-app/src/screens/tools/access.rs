//! Access tab: published pipeline callers and the honestly unavailable access rules (`lfvWC`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tools::data::ToolUse;

use super::editor::EditorProps;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::{note, section_title},
    notice::{Notice, NoticeTone},
    table::{ColumnWidth, table_header},
  },
};
const PIPELINE: f32 = 280.0;
const RESULT: f32 = 200.0;
const ROW: f32 = 56.0;
const OPEN: f32 = 14.0;

pub(super) fn access(ctx: &Ctx, p: &EditorProps) -> Column {
  let uses: Vec<_> = p.entry.iter().flat_map(|e| p.data.listed.used_by(e.id)).collect();
  let mut table = Column::new()
    .id("tool-access-steps")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(table_header(
      "tool-access-heading",
      &[
        (ColumnWidth::Fixed(PIPELINE), Some(&ctx.t("tools.columns.pipeline"))),
        (ColumnWidth::Fill, Some(&ctx.t("tools.columns.step"))),
        (ColumnWidth::Fixed(RESULT), Some(&ctx.t("tools.columns.result"))),
      ],
      true,
    ));
  if uses.is_empty() {
    table = table.child(
      Text::new(&ctx.t("tools.noUses"))
        .variant(TypographyStyle::Caption)
        .padding(SpacingSize::Md),
    );
  }
  for (i, used) in uses.iter().enumerate() {
    table = table.child(caller(ctx, i, used));
  }
  Column::new()
    .id("tool-access-tab")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .child(
      Column::new()
        .spacing(SpacingSize::Md)
        .child(section_title(
          "tool-access-uses-title",
          &ctx.t("tools.usedTitle"),
          Some(&ctx.t_args("tools.usedSteps", [("count", uses.len().to_string())])),
        ))
        .child(table)
        .child(note("tool-uses-note", Icon::Pin, &ctx.t("tools.usesNote"))),
    )
    .child(
      Column::new()
        .spacing(SpacingSize::Md)
        .child(section_title(
          "tool-rules-title",
          &ctx.t("tools.rulesTitle"),
          Some(&ctx.t("tools.rulesStatus")),
        ))
        .child(
          Notice::new("tool-access-rules", NoticeTone::Info, ctx.t("tools.accessTitle"))
            .body(ctx.t("tools.accessDetail"))
            .action(
              ActionButton::new("tool-access-edit", ButtonKind::Secondary, ctx.t("tools.editAccess"))
                .icon(Icon::SlidersHorizontal)
                .unavailable(ctx.t("tools.accessTitle")),
            ),
        ),
    )
}
fn caller(ctx: &Ctx, index: usize, used: &ToolUse) -> Row {
  // The pipeline editor is unavailable; show the real caller without a no-op Open control.
  Row::new()
    .id(format!("tool-access-step-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW))
    .spacing(SpacingSize::Md)
    .padding(Padding::horizontal(SpacingSize::Md))
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Column::new()
        .width(Dimension::Px(PIPELINE))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&used.pipeline_name)
            .variant(ExtraTypography::Strong.role())
            .nowrap(),
        )
        .child(
          Text::new(&ctx.t_args("tools.pipelineVersion", [("version", used.version.get().to_string())]))
            .variant(ExtraTypography::Note.role()),
        ),
    )
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::Terminal, IconSize::Md, PaletteColor::TextMuted))
        .child(Text::new(&used.step).variant(TypographyStyle::Body).nowrap()),
    )
    .child(
      Text::new(&ctx.t("tools.exitResult"))
        .variant(TypographyStyle::Description)
        .width(Dimension::Px(RESULT)),
    )
    .child(Row::new().width(Dimension::Px(OPEN)))
}
