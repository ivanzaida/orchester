//! A row of Review › Merges (`Row · KON-12` `y6btZh` in 09 `Os7b3`, every outcome in `Cjprb`) and the section that
//! holds rows: the 16 glyph of how the run stands, its task's key, title and run over its line, who and when, the
//! action that moves it on, and the chevron that opens the run's task. 60 high, 16 at its sides, 14 between its parts,
//! over a `border` line but for the section's last row. A section is its heading, 10 under the part above, with its
//! count in 12 `text_muted`, over its rows in one `border` box, radius 6.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::merge_attempts::merges::MergeRun;

use crate::{
  navigation::link::Open,
  theme::{DeliverySpacing, DeliveryText, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// A row's height while its line is one line (`y6btZh`).
const ROW_HEIGHT: f32 = 60.0;

/// What a row shows.
pub(super) struct Shown {
  pub id: String,
  /// Its glyph, in its color.
  pub glyph: (Icon, PaletteColor),
  /// Its line ([`line`]), and its name in the row's `outcome` description.
  pub line: (Element, &'static str),
  /// Who and when.
  pub meta: Arc<str>,
  pub action: Option<Element>,
  /// Where its chevron goes.
  pub opens: Open,
}

/// The line under a row's title: `text` in `color`, `<id>-line` for the row's `id`.
pub(super) fn line(id: &str, text: &str, color: PaletteColor) -> Element {
  Text::new(text)
    .id(format!("{id}-line"))
    .variant(ExtraTypography::Property.role())
    .color(color)
    .into()
}

/// The row of `run`, `last` of its section.
pub(super) fn row(ctx: &mut Ctx, run: &MergeRun, shown: Shown, last: bool) -> Element {
  let Shown {
    id,
    glyph,
    line,
    meta,
    action,
    opens,
  } = shown;
  let chevron = IconButton::new(format!("{id}-open"), Icon::ChevronRight, ctx.t("merges.action.openRun"))
    .compact()
    .opens(opens)
    .render(ctx);
  let mut row = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Sm))
    .spacing(DeliverySpacing::LaneRow.role())
    .align_items(Alignment::Center)
    .describe("outcome", line.1)
    .child(icon(glyph.0, IconSize::Lg, glyph.1).id(format!("{id}-glyph")))
    .child(words(ctx, &id, run, line.0))
    .child(
      Text::new(&meta)
        .id(format!("{id}-meta"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children(action)
    .child(chevron);
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  row.into()
}

/// The task's key, title and run (`Title line` `SVqHj`), and under them the line.
fn words(ctx: &Ctx, id: &str, run: &MergeRun, line: Element) -> Column {
  let number = ctx.t_args("merges.run", [("run", run.number.get().to_string())]);
  let title = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .min_width(Dimension::Px(0.0))
    .child(
      Text::new(&run.key.to_string())
        .variant(DeliveryText::Count.role())
        .nowrap(),
    )
    .child(
      Text::new(&run.title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::Strong.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&number)
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(title)
    .child(line)
}

/// A section: its heading with the count of `rows`, over them in their box.
pub(super) fn section(id: &str, title: &str, rows: Vec<Element>) -> Element {
  let count = rows.len().to_string();
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(
      Row::new()
        .id(format!("{id}-head"))
        .padding(Padding::new().top(DeliverySpacing::MergesSection.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(title).variant(ExtraTypography::FormSection.role()))
        .child(
          Text::new(&count)
            .id(format!("{id}-count"))
            .variant(ExtraTypography::Property.role())
            .color(PaletteColor::TextMuted),
        ),
    )
    .child(
      Column::new()
        .id(format!("{id}-rows"))
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .clip()
        .with_children(rows),
    )
    .into()
}
