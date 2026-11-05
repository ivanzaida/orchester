//! *Still changed in your checkout* (09l `Asu5i`): the files a bring-in that stopped part way left holding the merge's
//! version, each with *Holds the merge's version* in `warning`, and under them why *Try again* waits: until they match
//! the base branch again. *Put files back*, the page's primary action, puts them back.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::section_head;
use crate::{
  theme::{AttemptSpacing, DeliverySpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    unavailable::reason_line,
  },
};

/// The section of `files`, left changed by a merge into `base`.
pub(super) fn section(ctx: &Ctx, files: &[String], base: &str) -> Element {
  let rows: Vec<Element> = files
    .iter()
    .enumerate()
    .map(|(index, path)| {
      let mut row = Row::new()
        .id(format!("merge-attempt-left-{index}"))
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Lg, AttemptSpacing::CheckY.role()))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(icon(Icon::FileWarning, IconSize::Md, PaletteColor::Warning))
        .child(
          Text::new(path)
            .variant(ExtraTypography::PropertyMono.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .nowrap(),
        )
        .child(
          Text::new(&ctx.t("merges.page.holdsMerge"))
            .variant(ExtraTypography::Property.role())
            .color(PaletteColor::Warning)
            .nowrap(),
        );
      if index + 1 < files.len() {
        row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
      }
      row.into()
    })
    .collect();
  Column::new()
    .id("merge-attempt-left")
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(section_head(
      "merge-attempt-left-head",
      &ctx.t("merges.page.stillChanged"),
      Some(&files.len().to_string()),
    ))
    .child(
      Column::new()
        .id("merge-attempt-left-files")
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .clip()
        .with_children(rows),
    )
    .child(reason_line(
      "merge-attempt-try-again-reason",
      &ctx.t_args("merges.page.tryAgainWaits", [("branch", base)]),
    ))
    .into()
}
