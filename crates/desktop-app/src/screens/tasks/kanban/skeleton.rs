//! The board while the tasks are read (`17 · Loading` `Z1PMlo` in `Tasks board · redesign` `J6gQo`): every column of
//! the view keeps its real head, its status's icon and name, Fold and, in Backlog, Add a draft, with a bone for its
//! count and no Needs you hand yet; under it, card bones in the cards' own size and padding, on `surface_panel` in a
//! `border` stroke: the key and the age, the title, a run line (a dot and a bone) on a card that would have run, and
//! three chips. The last card of a column of three or more fades; Backlog keeps its *New task* foot. Columns by module
//! or pipeline, whose names come with the tasks, have bones for their names too.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::task::organization::{Priority, WorkflowStatus};

use super::{
  display::Columns,
  model::{owns, priority_name, status_name},
};
use crate::{
  routes::TaskListTab,
  theme::{ControlHeight, ExtraSpacing, IconSize, SKELETON_LAST_CARD},
  ui::{
    icon::{Icon, icon},
    skeleton::{bone, round_bone},
    status::{priority_look, workflow_key, workflow_look},
  },
};

/// A column's head (`Header` `VVQ0q`) and its count's bone (`FKa6e`).
const HEAD_HEIGHT: f32 = 28.0;
const COUNT: (f32, f32) = (14.0, 8.0);
/// A name's bone where the name comes with the tasks, and the status's round bone beside it.
const NAME: (f32, f32) = (56.0, 9.0);
const STATUS: f32 = 13.0;
/// A card bone's height: a draft's (`NyneD`) and a card with a run line's (`lLl6V`), as the cards they stand for.
const DRAFT_CARD: f32 = 101.0;
const RUN_CARD: f32 = 126.0;
/// The bones of a card: the key and the age (`Top` `xvizX`), the title's height, the run line's dot and height
/// (`Run` `bZydU`), and the chips (`Chips` `T0WDpp`).
const KEY: (f32, f32) = (38.0, 8.0);
const AGE: (f32, f32) = (14.0, 8.0);
const TITLE_HEIGHT: f32 = 11.0;
const RUN_DOT: f32 = 10.0;
const RUN_HEIGHT: f32 = 9.0;
const CHIPS: [f32; 3] = [18.0, 30.0, 52.0];
const CHIP_HEIGHT: f32 = 14.0;
/// *New task* at Backlog's foot (`za39y`).
const FOOT_HEIGHT: f32 = 28.0;
/// How many card bones each column shows, and the widths of the titles and run lines, so they do not look stamped.
const CARDS: [usize; 5] = [3, 4, 3, 3, 2];
const TITLES: [f32; 7] = [150.0, 120.0, 170.0, 135.0, 160.0, 110.0, 145.0];
const RUNS: [f32; 3] = [70.0, 110.0, 90.0];

/// A column of the skeleton: its name for ids, its look, and whether its cards would be drafts.
struct Head {
  name: String,
  look: Option<(Icon, PaletteColor, std::sync::Arc<str>)>,
  backlog: bool,
}

/// The skeleton of `view`'s board with its columns by `columns`.
pub fn board(ctx: &Ctx, (view, columns): (TaskListTab, Columns)) -> Column {
  let heads = heads(ctx, view, columns);
  let parts = heads.iter().enumerate().map(|(index, head)| column(ctx, head, index));
  Column::new()
    .id("tasks-loading-board")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::new().right(crate::theme::TasksSpacing::Sides.role()))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .with_children(parts),
    )
}

/// The columns the board will show before its tasks say more: the view's own statuses (Needs you, which owns none,
/// Active's, where what waits mostly is), every priority, or three columns whose names are not known yet.
fn heads(ctx: &Ctx, view: TaskListTab, columns: Columns) -> Vec<Head> {
  let view = match view {
    TaskListTab::NeedsYou => TaskListTab::Active,
    view => view,
  };
  match columns {
    Columns::Status => WorkflowStatus::ALL
      .into_iter()
      .filter(|status| owns(view, *status))
      .map(|status| {
        let (glyph, color) = workflow_look(status);
        Head {
          name: status_name(status).to_owned(),
          look: Some((glyph, color, ctx.t(workflow_key(status)))),
          backlog: status == WorkflowStatus::Backlog,
        }
      })
      .collect(),
    Columns::Priority => Priority::ALL
      .into_iter()
      .map(|priority| {
        let (glyph, word, color) = priority_look(priority);
        Head {
          name: priority_name(priority).to_owned(),
          look: Some((glyph, color, ctx.t(word))),
          backlog: false,
        }
      })
      .collect(),
    Columns::Module | Columns::Pipeline => (0..3)
      .map(|index| Head {
        name: index.to_string(),
        look: None,
        backlog: false,
      })
      .collect(),
  }
}

fn column(ctx: &Ctx, head: &Head, index: usize) -> Column {
  let id = format!("tasks-loading-column-{}", head.name);
  let count = CARDS[index % CARDS.len()];
  let cards = (0..count).map(|card| {
    let bones = card_bone(!head.backlog, index + card);
    if count >= 3 && card + 1 == count {
      bones.opacity(SKELETON_LAST_CARD)
    } else {
      bones
    }
  });
  Column::new()
    .id(id.clone())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(head_row(head, &id))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .with_children(cards)
        .with_children(head.backlog.then(|| new_task(ctx))),
    )
}

/// The column's head as the board draws it, a bone in its count's place; Fold and Backlog's Add a draft stay still
/// glyphs until the tasks are read.
fn head_row(head: &Head, id: &str) -> Row {
  let fold = still_glyph(format!("{id}-fold"), Icon::FoldHorizontal);
  let add = head.backlog.then(|| still_glyph(format!("{id}-add"), Icon::Plus));
  let row = Row::new()
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::new().left(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  let row = match &head.look {
    Some((glyph, color, label)) => row.child(icon(*glyph, IconSize::Base, color.clone())).child(
      Text::new(label)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    ),
    None => row.child(round_bone(STATUS)).child(bone(NAME.0, NAME.1)),
  };
  row
    .child(bone(COUNT.0, COUNT.1).id(format!("{id}-count")))
    .child(Spacer::new().flex(1.0))
    .child(fold)
    .with_children(add)
}

/// A head's control as a glyph in its 24 box, `text_muted`, which nothing presses yet (`Collapse` `r0Ph6y`).
fn still_glyph(id: String, glyph: Icon) -> Row {
  let side = ControlHeight::Sm.value();
  Row::new()
    .id(id)
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .flex_shrink(0.0)
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextMuted))
}

/// A card's bones in the card's frame, padding and height.
fn card_bone(runs: bool, seed: usize) -> Column {
  let line = || Row::new().width(Dimension::Pct(100.0)).align_items(Alignment::Center);
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(if runs { RUN_CARD } else { DRAFT_CARD }))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Inline.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Pair.role())
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      line()
        .spacing(ExtraSpacing::Snug.role())
        .child(bone(KEY.0, KEY.1))
        .child(Spacer::new().flex(1.0))
        .child(bone(AGE.0, AGE.1)),
    )
    .child(line().child(bone(TITLES[seed % TITLES.len()], TITLE_HEIGHT)))
    .with_children(runs.then(|| {
      line()
        .spacing(ExtraSpacing::Snug.role())
        .child(round_bone(RUN_DOT))
        .child(bone(RUNS[seed % RUNS.len()], RUN_HEIGHT))
    }))
    .child(
      line()
        .spacing(SpacingSize::Xs)
        .with_children(CHIPS.map(|width| bone(width, CHIP_HEIGHT))),
    )
}

/// Backlog's *New task* foot, as the board draws it.
fn new_task(ctx: &Ctx) -> Row {
  Row::new()
    .id("tasks-loading-new-task")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOT_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Plus, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("tasks.newTask"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}
