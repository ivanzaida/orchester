//! *Still changed in your checkout* (13h `anuAO`): the files a bring-in that stopped part way left holding the merge's
//! version, each with *Holds the merge's version* in `warning`. Why *Try again* waits, and *Put files back*, are in the
//! bar.

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
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// The section of `files`, left changed by a merge.
pub(super) fn section(ctx: &Ctx, files: &[String]) -> Element {
  let rows: Vec<Element> = files
    .iter()
    .enumerate()
    .map(|(index, path)| {
      let mut row = Row::new()
        .id(format!("merge-attempt-left-{index}"))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ControlHeight::Lg.value()))
        .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Warning))
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
    .spacing(SpacingSize::Sm)
    .child(section_head(
      "merge-attempt-left-head",
      &ctx.t("merges.page.stillChanged"),
      (Some(&files.len().to_string()), None),
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
    .into()
}
