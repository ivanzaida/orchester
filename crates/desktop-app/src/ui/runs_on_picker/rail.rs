//! The compact picker's rail (`Rail` `tsFtb` in `ZcGGy`): the connections in a row, 2 apart, grouped by their marks
//! with a thin divider between groups (`kFLF6`: 9 wide, a 1 × 14 `border` line). Each is a 28 square of radius `md`
//! with its 14 mark; the shown one on `pressed` under a 12 × 2 `accent` bar (`yTnqv`), where the keys are on `hover`, a
//! plan at its limit with a 6 `warning` dot ringed in `surface_raised` at its top right (`V7gbO`), one that cannot be
//! chosen at 40 % and inert, where the keys are also in the focus ring. A tooltip names each, or says why it
//! cannot be chosen. Past the 288 it has, the rail scrolls sideways and keeps the shown item and the keys' in view.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::ScrollEvent,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Button, Column, Row, ScrollHorizontal, Stack},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
    scrollbar::ScrollBarStyle,
  },
  node::{Element, Style, border::Border, dimension::Dimension},
};

use super::model::{Connection, Mark};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, IconSize, RUNS_ON_UNAVAILABLE_OPACITY},
  ui::{
    icon::{Icon, icon},
    pressable::{inert, pressable},
    tooltip::Tooltip,
  },
};

/// An item's side (`eKfKm`), the shown one's bar (`yTnqv`) and the limit dot (`V7gbO`) with where they sit in it.
const ITEM: f32 = 28.0;
const BAR_LEFT: f32 = 8.0;
const BAR_TOP: f32 = 26.0;
const BAR_WIDTH: f32 = 12.0;
const BAR_HEIGHT: f32 = 2.0;
const DOT: f32 = 6.0;
const DOT_LEFT: f32 = 19.0;
const DOT_TOP: f32 = 4.0;
/// A group divider's width (`kFLF6`) and its line's height (`yeCEI`).
const DIVIDER_WIDTH: f32 = 9.0;
const DIVIDER_LINE: f32 = 14.0;

/// What choosing the connection at a place on the rail does.
pub(super) type Choose = Arc<dyn Fn(usize) + Send + Sync>;

/// The room the rail has: the picker's width less the head's padding at each side.
fn visible() -> f32 {
  super::WIDTH - 2.0 * ExtraSpacing::Snug.value()
}

/// Where the connection at `index` starts along the rail: every item before it, and every divider between groups
/// before it, with the gaps between them all.
fn start_of(connections: &[Connection], index: usize) -> f32 {
  let gap = ExtraSpacing::Tight.value();
  let dividers = connections[..=index]
    .windows(2)
    .filter(|pair| pair[0].mark != pair[1].mark)
    .count();
  // A rail of a few dozen items: the casts are exact.
  index as f32 * (ITEM + gap) + dividers as f32 * (DIVIDER_WIDTH + gap)
}

/// Scrolls the rail held by `scroll` so that the connection at `index` shows whole. A rail that fits does not scroll.
pub(super) fn reveal(scroll: &ScrollState, connections: &[Connection], index: usize) {
  if index >= connections.len() {
    return;
  }
  let (start, room) = (start_of(connections, index), scroll.viewport_width());
  let room = if room > 0.0 { room } else { visible() };
  let shown = scroll.scroll_x();
  if start < shown {
    scroll.set_scroll_pending(start, 0.0);
  } else if start + ITEM > shown + room {
    scroll.set_scroll_pending(start + ITEM - room, 0.0);
  }
}

/// The rail of `connections`: the one at `viewing` shown, the keys at `cursor`. Past what fits it scrolls sideways
/// under a hidden scrollbar: by the wheel (either axis) or a swipe, and by the keys, which keep their item in view.
pub(super) fn rail(
  ctx: &mut Ctx,
  (id, scroll): (&str, &ScrollState),
  connections: &[Connection],
  (viewing, cursor): (Option<usize>, Option<usize>),
  choose: &Choose,
) -> Element {
  let mut items: Vec<Element> = Vec::new();
  for (index, connection) in connections.iter().enumerate() {
    if index > 0 && connections[index - 1].mark != connection.mark {
      items.push(divider());
    }
    let state = (viewing == Some(index), cursor == Some(index));
    items.push(item(ctx, (id, index, connection), state, choose));
  }
  let row = Row::new()
    .id(format!("{id}-rail"))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .with_children(items);
  let state = scroll.clone();
  ScrollHorizontal::new(row)
    .with_scroll_state(scroll.clone())
    .id(format!("{id}-rail-scroll"))
    .scrollbar(ScrollBarStyle::hidden())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ITEM))
    .on_scroll(move |event: ScrollEvent| {
      // lurq's sideways scroller leaves a vertical wheel to the containers around it: the rail takes the stronger
      // delta.
      let most = (state.content_width() - state.viewport_width()).max(0.0);
      if most <= 0.0 {
        return;
      }
      event.prevent_default();
      let delta = if event.delta_y.abs() > event.delta_x.abs() {
        event.delta_y
      } else {
        event.delta_x
      };
      state.set_scroll_pending((state.scroll_x() - delta).clamp(0.0, most), 0.0);
    })
    .into()
}

fn divider() -> Element {
  Row::new()
    .width(Dimension::Px(DIVIDER_WIDTH))
    .height(Dimension::Px(ITEM))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(1.0))
        .height(Dimension::Px(DIVIDER_LINE))
        .background(PaletteColor::Border),
    )
    .into()
}

/// The connection at `index`, `shown` and where the keys are (`current`) as said.
fn item(
  ctx: &mut Ctx,
  (id, index, connection): (&str, usize, &Connection),
  (shown, current): (bool, bool),
  choose: &Choose,
) -> Element {
  let mark = match connection.mark {
    Mark::Agent(mark) => mark.render(ctx, IconSize::Md),
    Mark::Cloud => icon(Icon::Cloud, IconSize::Md, PaletteColor::TextSecondary).into(),
  };
  let choosable = connection.unavailable.is_none();
  let button = if choosable {
    let choose = choose.clone();
    pressable(move || choose(index)).hovered_style(Style::new().background(ExtraColor::Hover.role()))
  } else {
    inert(Button::empty()).opacity(RUNS_ON_UNAVAILABLE_OPACITY)
  };
  let tip: Arc<str> = connection.unavailable.clone().unwrap_or_else(|| connection.tip.clone());
  let button = button
    .id(format!("{id}-connection-{}", connection.id))
    .describe("chosen", shown.to_string())
    .describe("current", current.to_string())
    .describe("choosable", choosable.to_string())
    .describe("limit", connection.limit.to_string())
    .describe("a11y_name", tip.clone())
    .width(Dimension::Px(ITEM))
    .height(Dimension::Px(ITEM))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(mark);
  let button = match (shown, current) {
    (true, _) => button.background(ExtraColor::Pressed.role()),
    (false, true) => button.background(ExtraColor::Hover.role()),
    (false, false) => button,
  };
  // Only the keys put the cursor here, so it is the focus ring's place.
  let button = if current {
    button.box_shadow(ExtraShadow::FocusRing.role()).class("ringed")
  } else {
    button
  };
  // A stack, as only a stack lays out what is placed at a point of it.
  let item = Stack::new()
    .id(format!("{id}-connection-{}-item", connection.id))
    .child(button)
    .with_children(connection.limit.then(|| limit_dot(id, &connection.id)))
    .with_children(shown.then(|| shown_bar(id, &connection.id)));
  let tooltip = Tooltip::new(format!("{id}-connection-{}-tip", connection.id), tip);
  let tooltip = match &connection.tip_note {
    Some(note) => tooltip.note(note.clone()),
    None => tooltip,
  };
  tooltip.wrap(ctx, item)
}

/// The `warning` dot of a plan at its limit, at the item's top right.
fn limit_dot(id: &str, connection: &str) -> Element {
  Row::new()
    .id(format!("{id}-connection-{connection}-limit"))
    .absolute(DOT_LEFT, DOT_TOP, Dimension::Px(DOT), Dimension::Px(DOT))
    .pointer_events_none()
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Warning)
    .border(Border::inside(BorderSize::Md, PaletteColor::SurfaceRaised))
    .into()
}

/// The `accent` bar under the shown item.
fn shown_bar(id: &str, connection: &str) -> Element {
  Column::new()
    .id(format!("{id}-connection-{connection}-bar"))
    .absolute(BAR_LEFT, BAR_TOP, Dimension::Px(BAR_WIDTH), Dimension::Px(BAR_HEIGHT))
    .pointer_events_none()
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::Accent)
    .into()
}
