//! What Review › Merges shows in place of its sections. Empty (09c `jhPCL`): the State/Empty card, 440 wide and
//! centered under the info line, with the git-merge tile, *No merges yet* and what will show here; the tab has no
//! count. While the merges are read (09d `Zlk49`): the info line, its lead naming the checkout's branch and its values
//! bones, and after the skeleton's delay as the other Review tabs, a section heading's bones and one box of row bones
//! in the rows' own geometry (glyph, title and line, who and when, action); which sections exist is data. *Reading the
//! merges…* (`merges.reading`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::header;
use crate::{
  theme::{DeliverySpacing, ExtraTypography, IconSize},
  ui::{
    icon::Icon,
    skeleton::{self, Skeleton, fading, round_bone},
    state::EmptyState,
  },
};

/// The empty state's card (`jhPCL`).
const EMPTY_WIDTH: f32 = 440.0;
/// A reading row's height and its bones: its title's and its line's, who and when, and its action (`Zlk49`).
const ROW_HEIGHT: f32 = 60.0;
const ROWS: [(f32, f32); 3] = [(260.0, 400.0), (200.0, 340.0), (300.0, 440.0)];
const META: f32 = 110.0;
const ACTION: f32 = 64.0;
/// A section heading's title bone (`Zlk49`).
const HEADING: f32 = 80.0;

/// No merge was ever tried: *No merges yet* under the info line, `branch` the one the checkout has.
pub(super) fn empty(ctx: &Ctx, branch: Option<&str>) -> Element {
  let body = match branch {
    Some(branch) => ctx.t_args("merges.empty.body", [("branch", branch)]),
    None => ctx.t("merges.empty.bodyNoBranch"),
  };
  let card = EmptyState::new("merges-empty", Icon::GitMerge, ctx.t("merges.empty.title"), body).wide();
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(DeliverySpacing::MergesEmpty.role()))
    .align_items(Alignment::Center)
    .child(Column::new().width(Dimension::Px(EMPTY_WIDTH)).child(card))
    .into()
}

/// The page while the merges are read; `branch`, the one the checkout has, once git named it. The info line stands
/// outside the pulse, its lead at full colour, only its values' bones in it.
pub(super) fn reading(ctx: &mut Ctx, branch: Option<&str>) -> Element {
  let rows = ROWS.iter().map(|widths| row(ctx, *widths)).collect();
  let heading = Row::new()
    .padding(Padding::new().top(DeliverySpacing::MergesSection.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(skeleton::line(ctx, ExtraTypography::FormSection.role(), HEADING))
    .child(skeleton::count());
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(heading)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .with_children(fading(rows)),
    );
  let info = header::reading(ctx, branch);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(DeliverySpacing::Lanes.role())
    .child(info)
    .child(Skeleton::new("merges-loading", ctx.t("merges.reading")).show(ctx, bones))
    .into()
}

/// A row's bones.
fn row(ctx: &Ctx, (title, line): (f32, f32)) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(DeliverySpacing::LaneRow.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(round_bone(IconSize::Lg.size()))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(skeleton::line(ctx, ExtraTypography::Strong.role(), title))
        .child(skeleton::line(ctx, ExtraTypography::Property.role(), line)),
    )
    .child(skeleton::line(ctx, ExtraTypography::Property.role(), META))
    .child(skeleton::button(ACTION))
}
