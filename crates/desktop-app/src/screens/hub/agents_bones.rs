//! The Agents hub's body while it is read for the first time (`Agents · v2` `acFkz`; `Loading states · configure
//! screens` `psFv5`): the list and the panel in bones, in the master-detail's outline, where the real ones will stand.
//! The list is a group's head over rows of the real rows' height ([`ROW_HEIGHT`]), the lower ones fading; the panel is
//! a head, its chips, a section of rows and the pinned bar. A team's and a local project's hub share it: until the hub
//! is read neither knows what the chosen entry is.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{agent_row::ROW_HEIGHT, agents::master_detail};
use crate::{
  screens::team_hub::{bar::bar_named, list::SECTION_HEIGHT, panel, rows},
  theme::{ExtraRadius, ExtraSpacing, HubText, skeleton_line},
  ui::skeleton::{self, Skeleton, bone, fading},
};

/// A row's glyph (`IconSize::Lg`, 16).
const GLYPH: f32 = 16.0;
/// The list's rows in bones: how long each one's name and line are.
const LIST: [(f32, f32); 6] = [
  (120.0, 180.0),
  (96.0, 150.0),
  (140.0, 200.0),
  (108.0, 160.0),
  (88.0, 140.0),
  (128.0, 170.0),
];
/// The version at a row's end, the group's title, the panel's head (what it is, its name, the button at its end) and
/// its section's label.
const VERSION: f32 = 18.0;
const GROUP: f32 = 72.0;
const HEAD: (f32, f32, f32) = (96.0, 180.0, 88.0);
const SECTION: f32 = 64.0;
/// The chips under the head.
const CHIPS: [f32; 2] = [36.0, 72.0];
/// The panel's rows in bones: how long each one's key and value are.
const ROWS: [(f32, f32); 4] = [(72.0, 140.0), (96.0, 120.0), (64.0, 160.0), (88.0, 96.0)];
/// A row's field, and the bar's *Save* and *Cancel*.
const FIELD: f32 = 140.0;
const BAR: [f32; 2] = [72.0, 80.0];

/// The hub's body in bones under `id`, named `label` (the hub's reading words).
pub(crate) fn body(ctx: &mut Ctx, id: &str, label: Arc<str>) -> Element {
  let bones = master_detail(id, true, (list(ctx), detail(ctx, id)));
  Skeleton::new(id.to_owned(), label).fills().show(ctx, bones)
}

/// A group's head (its title and count) over the rows.
fn list(ctx: &Ctx) -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(skeleton::line(ctx, HubText::GroupTitle.role(), GROUP))
    .child(skeleton::count());
  let rows = fading(LIST.iter().map(|widths| row(ctx, *widths)).collect());
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .child(head)
    .with_children(rows)
    .into()
}

/// A row of the list (`hub::agent_row`): the glyph, the name over its line, and the version at its end.
fn row(ctx: &Ctx, (name, line): (f32, f32)) -> Row {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(skeleton::line(ctx, HubText::RowName.role(), name))
    .child(skeleton::line(ctx, HubText::RowLine.role(), line));
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(bone(GLYPH, GLYPH))
    .child(text)
    .child(skeleton::line(ctx, HubText::RowLine.role(), VERSION))
}

/// The panel (`team_hub::panel`, `local_hub::panel`): the head, the chips and a section over the pinned bar.
fn detail(ctx: &Ctx, id: &str) -> Element {
  let (kind, name, end) = HEAD;
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(skeleton::line(ctx, HubText::PanelWhere.role(), kind))
        .child(skeleton::line(ctx, HubText::PanelTitle.role(), name)),
    )
    .child(skeleton::button(end));
  let chips = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(CHIPS.map(|width| chip(ctx, width)));
  let rows = ROWS
    .iter()
    .enumerate()
    .map(|(index, (key, value))| {
      rows::keyed(
        &format!("{id}-row-{index}"),
        skeleton::line(ctx, HubText::Status.role(), *key),
        None,
        skeleton::field(ctx, FIELD, *value).width(Dimension::Pct(100.0)),
      )
    })
    .collect();
  let label = skeleton::line(ctx, HubText::PanelLabel.role(), SECTION).into();
  let section = rows::section_headed(&format!("{id}-section"), label, Vec::new(), rows);
  let buttons = BAR.map(|width| skeleton::button(width).into()).to_vec();
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .clip()
        .child(panel::body().child(head).child(chips).child(section)),
    )
    .child(bar_named(&format!("{id}-actions"), buttons, None))
    .into()
}

/// A chip (`ui::chip::badge`) whose words are not read yet: its pill, as long as words `width` long.
fn chip(ctx: &Ctx, width: f32) -> Row {
  let (line_height, _) = skeleton_line(ctx.theme(), TypographyStyle::FieldLabel);
  Row::new()
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Fine.role()))
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Border)
    .child(
      Row::new()
        .width(Dimension::Px(width))
        .height(Dimension::Px(line_height)),
    )
}
