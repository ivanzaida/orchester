//! A decision card's facts while they are read: their room from the start, so the card keeps its height when they
//! arrive, and after [`SKELETON_DELAY`](crate::theme::SKELETON_DELAY) a bone for each key and value (`ui::skeleton`,
//! as the Tasks skeleton's text bones), pulsing gently. Each row is as high as the fact it stands for: a value's line,
//! and a link's underline under it where the fact will be a link.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{border::Border, dimension::Dimension},
};
use orchester_core::orchestrator::change::Change;

use super::{super::decision::FACT_KEY_WIDTH as KEY_WIDTH, words::View};
use crate::{
  theme::{ExtraSpacing, OrchestratorText},
  ui::skeleton::{bone, pulsing},
};

/// A bone's height, as a line of 12 text (`C8Z6Wd`'s key bone), a key's width and each value's.
const BONE_HEIGHT: f32 = 9.0;
const KEY_BONE: f32 = 44.0;
const VALUE_BONES: [f32; 3] = [168.0, 132.0, 148.0];
/// A blank line's text: no words, only a line's height.
const BLANK: &str = " ";

/// The facts a card of `view` will show, before they are read, by whether each holds a link: a merge's run (a link to
/// its task's Summary), its branch and its changes (a link to the task's Changes); nothing for the other cards, whose
/// facts depend on what is read.
pub(super) fn shape(view: &View) -> Vec<bool> {
  let task = view.action.task.is_some();
  match view.action.change.as_ref().map(|change| &change.change) {
    Some(Change::Merge { .. }) => vec![task, false, task],
    _ => Vec::new(),
  }
}

/// The room of facts shaped as `links` says, in card `id`; their bones once `shown`. Nothing without facts to stand
/// for.
pub(super) fn column(id: &str, links: &[bool], shown: bool) -> Option<Column> {
  if links.is_empty() {
    return None;
  }
  let rows = links
    .iter()
    .zip(VALUE_BONES.iter().cycle())
    .enumerate()
    .map(|(index, (link, value))| row(format!("{id}-facts-bone-{index}"), *link, shown.then_some(*value)));
  let room = Column::new()
    .id(format!("{id}-facts-reading"))
    .describe("state", if shown { "skeleton" } else { "waiting" })
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .with_children(rows);
  Some(if shown { pulsing(room) } else { room })
}

/// One fact's room: an empty line of a fact's text, with a link's underline under it for a `link`, holds its height;
/// the key's and the value's bones, `value` wide, when shown, the value's named `bone_id`.
fn row(bone_id: String, link: bool, value: Option<f32>) -> Row {
  let line = Column::new().width(Dimension::Px(0.0)).child(
    Text::new(BLANK)
      .variant(OrchestratorText::FactValue.role())
      .width(Dimension::Px(0.0)),
  );
  let line = if link {
    line
      .padding_bottom(ExtraSpacing::Hair.role())
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
  } else {
    line
  };
  let key = Row::new()
    .width(Dimension::Px(KEY_WIDTH))
    .flex_shrink(0.0)
    .with_children(value.map(|_| bone(KEY_BONE, BONE_HEIGHT)));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(key)
    .with_children(value.map(|value| bone(value, BONE_HEIGHT).id(bone_id)))
    .child(line)
}
