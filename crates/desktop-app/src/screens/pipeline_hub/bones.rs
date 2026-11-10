//! The Pipelines page's body while its pipelines are read for the first time (`Pipelines · v2` P5 `TF9O3`): the list
//! and the panel in bones, in the master-detail's outline, where the real ones will stand. The title, *New pipeline*
//! and the tab names stay real around them ([`super`]'s frame), the counts are bones ([`agents::Tab`]'s `None`).
//!
//! The list is a group's head over four rows of the real rows' height, the lower ones fading; the panel ([`detail`]) is
//! a head, its chips and the steps as the flow they run in, each a card on its rail, the lower ones fading.
//! This page has no read of a single pipeline, so bones appear only for the whole body.

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

use crate::{
  screens::{
    hub::{agent_row::ROW_HEIGHT, agents::master_detail},
    team_hub::{list::SECTION_HEIGHT, panel},
  },
  theme::{ControlHeight, ExtraRadius, ExtraSpacing, HubText, PIPELINES_BONE_FADE},
  ui::skeleton::{Skeleton, bone, round_bone},
};

/// The skeleton's element id; the master-detail inside it is `<id>-master-detail`.
const ID: &str = "pipeline-hub-loading";

/// A row's mark (`Mark bone` `wHL5s`) and its version at the end (`QaY1H`).
const MARK: f32 = 16.0;
const VERSION: (f32, f32) = (18.0, 8.0);
/// The group's title (`c8L0h`).
const GROUP: (f32, f32) = (70.0, 8.0);
/// The rows of the list: how long each one's name and line are, the name 9 high and the line 8 (`pfcfi`, `CoQtm`).
const ROWS: [(f32, f32); 4] = [(120.0, 170.0), (96.0, 150.0), (130.0, 180.0), (90.0, 140.0)];
const NAME_HEIGHT: f32 = 9.0;
const LINE_HEIGHT: f32 = 8.0;
/// The panel's head: what it is, 8 high, and its name, 12 high (`l9OV9s`, `y9AAT`).
const HEAD: [(f32, f32); 2] = [(130.0, 8.0), (190.0, 12.0)];
/// The chips under the head, 18 high and round (`e7nNK`, `t5tqG`).
const CHIPS: [f32; 2] = [36.0, 58.0];
const CHIP_HEIGHT: f32 = 18.0;
/// The count beside *Steps* (`WM24f`).
const STEPS_COUNT: (f32, f32) = (10.0, 8.0);
/// The steps: how long each card's name and line are (`m4wX9`, `Snm2F`).
const STEPS: [(f32, f32); 4] = [(90.0, 210.0), (70.0, 180.0), (110.0, 230.0), (80.0, 160.0)];
/// A step's rail (`Rail` `oBUKk`): its 22 node with a 1 wide line 5 above it and 28 below, in a 55 high step.
const RAIL: f32 = 22.0;
const RAIL_LINE: f32 = 1.0;
const RAIL_ABOVE: f32 = 5.0;
const STEP_HEIGHT: f32 = 55.0;

/// The page's body in bones under [`ID`], named `label` (the page's reading words).
pub(super) fn body(ctx: &mut Ctx, label: Arc<str>) -> Element {
  let bones = master_detail(ID, true, (list(), detail(ctx)));
  Skeleton::new(ID, label).fills().show(ctx, bones)
}

/// The list: the group's head over the rows, the lower rows fading.
fn list() -> Element {
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .align_items(Alignment::Center)
    .child(bone(GROUP.0, GROUP.1));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .child(head)
    .with_children(
      ROWS
        .iter()
        .enumerate()
        .map(|(index, widths)| row(*widths).opacity(PIPELINES_BONE_FADE[index])),
    )
    .into()
}

/// A row of the list (`pipeline_hub::row`): the mark, the name over its line, and the version at its end.
fn row((name, line): (f32, f32)) -> Row {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::TextArea.role())
    .child(bone(name, NAME_HEIGHT))
    .child(bone(line, LINE_HEIGHT));
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(bone(MARK, MARK))
    .child(text)
    .child(bone(VERSION.0, VERSION.1))
}

/// The panel of a pipeline not read yet: its head, its chips and its steps.
fn detail(ctx: &Ctx) -> Element {
  let [(kind, kind_height), (name, name_height)] = HEAD;
  let head = Column::new()
    .spacing(ExtraSpacing::Inline.role())
    .child(bone(kind, kind_height))
    .child(bone(name, name_height));
  let chips = Row::new()
    .spacing(SpacingSize::Sm)
    .with_children(CHIPS.map(|width| bone(width, CHIP_HEIGHT).rounded(ExtraRadius::Full.role())));
  let body = panel::body()
    .spacing(ExtraSpacing::SettingsHead.role())
    .child(head)
    .child(chips)
    .child(steps(ctx));
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .clip()
    .child(body)
    .into()
}

/// *Steps* with the count's bone over the flow's steps.
fn steps(ctx: &Ctx) -> Column {
  let head = Row::new()
    .height(Dimension::Px(ControlHeight::Md.value()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.panel.steps"))
        .variant(HubText::PanelLabel.role())
        .nowrap(),
    )
    .child(bone(STEPS_COUNT.0, STEPS_COUNT.1));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(head)
    .child(
      Column::new().width(Dimension::Pct(100.0)).with_children(
        STEPS
          .iter()
          .enumerate()
          .map(|(index, widths)| step(*widths).opacity(PIPELINES_BONE_FADE[index])),
      ),
    )
}

/// A step: its rail with the node, and its card.
fn step((name, line): (f32, f32)) -> Row {
  let rail_line = || {
    Column::new()
      .width(Dimension::Px(RAIL_LINE))
      .background(PaletteColor::BorderStrong)
  };
  let rail = Column::new()
    .width(Dimension::Px(RAIL))
    .height(Dimension::Px(STEP_HEIGHT))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .child(rail_line().height(Dimension::Px(RAIL_ABOVE)))
    .child(round_bone(RAIL))
    .child(rail_line().flex(1.0));
  let card = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::TiledRow.role()))
    .spacing(SpacingSize::Sm)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(bone(name, NAME_HEIGHT))
    .child(bone(line, LINE_HEIGHT));
  let cell = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::new().bottom(SpacingSize::Sm))
    .child(card);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STEP_HEIGHT))
    .spacing(SpacingSize::Md)
    .child(rail)
    .child(cell)
}
