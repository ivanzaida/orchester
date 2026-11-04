//! How the chat shows what the search found (13f `aoVlE`, *Marks*): a place on `warning_muted` with its text in its
//! own color, the current one on `warning` in `text_inverse`, radius 4 (`Match`, `Match · current`); a folded step and
//! a closed tool group count the places inside them in a `Badge/Warning` without its dot, 1 over and under and 6 at
//! its sides, 11/500 `warning` (`Matches` `OUbMr`); what the search opened says so, a 12 `search` in `warning` and
//! 11 `text_muted` 4 apart (`Opened by search` `f76I4`); the chat's track carries a mark where each place sits in the
//! whole run, the current one brighter, a click goes to it (`Match marks` `z4lnO5`: a 6 wide `hover` track, radius
//! full, 16 in from the chat's top, end and foot; a mark 2 high, the current one 4, `warning`, radius 1).
//!
//! A message, the person's, an agent's or the Orchestrator's, fills behind its matched words itself
//! ([`Marks::in_said`], `ui::chat::found`). A card, a problem or a notice says several texts, and a place found in it
//! does not say in which: its row is marked instead, a 2 rule at its start, `warning` for the current place.

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::timeline::find::{Hit, Needle, Spot};

use super::run::Found;
use crate::{
  theme::{ConversationRadius, ExtraColor, ExtraRadius, ExtraSpacing, IconSize, TRACK_MARK_OPACITY},
  ui::{
    chat::found::Found as InMessage,
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The track's width and a mark's height, the current one's (`z4lnO5`).
const TRACK_WIDTH: f32 = 6.0;
const MARK_HEIGHT: f32 = 2.0;
const CURRENT_MARK_HEIGHT: f32 = 4.0;
/// The track's marks, at most: places closer than this share one.
const TRACK_MARKS: usize = 200;

/// What the chat's rows mark: what the search looked for, the places it found, and which is current. Rows compare it
/// by the search that found the places and the current one.
#[derive(Clone, Debug)]
pub(in super::super) struct Marks {
  needle: Option<Needle>,
  found: Arc<Found>,
  current: Option<usize>,
}

impl PartialEq for Marks {
  fn eq(&self, other: &Self) -> bool {
    self.needle == other.needle && Arc::ptr_eq(&self.found, &other.found) && self.current == other.current
  }
}

/// A place in a call's line or output: its bytes, and whether it is the current one.
pub(in super::super) type Place = (Range<usize>, bool);

impl Marks {
  pub(in super::super) const fn new(needle: Option<Needle>, found: Arc<Found>, current: Option<usize>) -> Self {
    Self { needle, found, current }
  }

  /// How a row that shows the cards or notices `items` is marked for what they say: `Some(true)` holding the current
  /// place, `Some(false)` another, `None` none.
  pub(in super::super) fn row(&self, items: &[&str]) -> Option<bool> {
    let said = |item: &&str| self.found.said_in(item);
    let current = self
      .current
      .and_then(|at| self.found.hits.get(at))
      .is_some_and(|hit| hit.spot == Spot::Said && items.contains(&&*hit.item));
    current
      .then_some(true)
      .or_else(|| items.iter().any(said).then_some(false))
  }

  /// What a message, the item `item`, fills behind its words: the needle, and the rank of the current place among
  /// the places found in what it says. `None` when it says none.
  pub(in super::super) fn in_said(&self, item: &str) -> Option<InMessage> {
    let needle = self.needle.clone()?;
    let said: Vec<usize> = self
      .found
      .in_item(item)
      .iter()
      .copied()
      .filter(|&index| self.found.hits[index].spot == Spot::Said)
      .collect();
    if said.is_empty() {
      return None;
    }
    let current = self.current.and_then(|at| said.iter().position(|&index| index == at));
    Some(InMessage::new(needle, current))
  }

  /// How many places the folds of the steps `steps` hide.
  pub(in super::super) fn in_steps<'a>(&self, steps: impl IntoIterator<Item = &'a Arc<str>>) -> usize {
    steps.into_iter().map(|step| self.found.in_step(step)).sum()
  }

  /// How many places the tool group `group` holds.
  pub(in super::super) fn in_group(&self, group: &str) -> usize {
    self.found.in_item(group).len()
  }

  /// The places in the call `call` of the tool group `group` at `spot`.
  pub(in super::super) fn in_call(
    &self,
    (group, call): (&str, &str),
    spot: impl Fn(Spot) -> bool,
  ) -> Vec<(Spot, Place)> {
    self
      .found
      .in_item(group)
      .iter()
      .map(|&index| (index, &self.found.hits[index]))
      .filter(|(_, hit)| hit.call.as_ref().is_some_and(|held| &*held.key == call) && spot(hit.spot))
      .map(|(index, hit)| (hit.spot, (hit.range.clone(), self.current == Some(index))))
      .collect()
  }
}

/// `row`, a row of the chat, with the rule at its start that marks what it says: `warning` for the current place
/// (its element id is `conversation-search-current-row`), `warning_muted` for another (`conversation-search-row`).
pub(in super::super) fn row_mark<T: Into<Element>>(row: T, mark: Option<bool>) -> Element {
  let Some(current) = mark else {
    return row.into();
  };
  let (id, color) = if current {
    ("conversation-search-current-row", PaletteColor::Warning)
  } else {
    ("conversation-search-row", PaletteColor::WarningMuted)
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .border_left(Border::inside(BorderSize::Md, color))
    .child(row)
    .into()
}

/// `text` on one line in `style` and `color`, the places found in it marked; `base` is where in `text` the text the
/// places count their bytes in starts. The current place's element id is `conversation-search-current`, another's
/// `conversation-search-match`.
pub(in super::super) fn marked(
  text: &str,
  (style, color): (TypographyStyle, PaletteColor),
  (base, places): (usize, &[Place]),
) -> Vec<Element> {
  let mut pieces = Vec::new();
  let mut at = 0;
  let mut places: Vec<(Range<usize>, bool)> = places
    .iter()
    .map(|(range, current)| (range.start + base..range.end + base, *current))
    .filter(|(range, _)| {
      range.end <= text.len() && text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
    })
    .collect();
  places.sort_by_key(|(range, _)| range.start);
  for (range, current) in places {
    if range.start < at {
      continue;
    }
    pieces.extend(piece(&text[at..range.start], (style, &color), None));
    pieces.extend(piece(&text[range.clone()], (style, &color), Some(current)));
    at = range.end;
  }
  pieces.extend(piece(&text[at..], (style, &color), None));
  pieces
}

/// A piece of a marked line: plain, or a place found (`Some(true)` the current one).
fn piece(text: &str, (style, color): (TypographyStyle, &PaletteColor), found: Option<bool>) -> Option<Element> {
  if text.is_empty() {
    return None;
  }
  // A piece's spaces at its ends are its own: a space that ends a line does not collapse.
  let words = Text::new(&text.replace(' ', "\u{a0}")).variant(style).nowrap();
  Some(match found {
    None => words.color(color.clone()).into(),
    Some(current) => {
      let (id, fill, color) = if current {
        (
          "conversation-search-current",
          PaletteColor::Warning,
          PaletteColor::TextInverse,
        )
      } else {
        ("conversation-search-match", PaletteColor::WarningMuted, color.clone())
      };
      Row::new()
        .id(id)
        .background(fill)
        .rounded(RadiusSize::Sm)
        .child(words.color(color))
        .into()
    }
  })
}

/// *4 matches* (`Matches` `OUbMr`), for `count` places; `id` names it.
pub(in super::super) fn count_badge(ctx: &Ctx, id: &str, count: usize) -> Element {
  let words = if count == 1 {
    ctx.t("conversation.search.matches.one")
  } else {
    ctx.t_args(
      "conversation.search.matches.other",
      [("count", count.to_string().as_str())],
    )
  };
  Row::new()
    .id(id.to_owned())
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::WarningMuted)
    .rounded(ExtraRadius::Full.role())
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Link)
        .color(PaletteColor::Warning)
        .nowrap(),
    )
    .into()
}

/// *Opened by search* (`f76I4`), on what the search opened to show its current place; `id` names it.
pub(in super::super) fn opened(ctx: &Ctx, id: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(Icon::Search, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t("conversation.search.opened"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// The chat's track of marks: where each place sits among the chat's `rows`, by the row (`row_of`) that shows it; a
/// click on a mark makes its first place current (`go`).
pub(in super::super) fn track(
  (hits, current): (&[Hit], Option<usize>),
  rows: usize,
  row_of: &dyn Fn(&Hit) -> Option<usize>,
  go: &(dyn Fn(usize) -> Box<dyn Fn() + Send + Sync> + Sync),
) -> Element {
  // Each place's share of the way down, and the first place of each of the track's marks.
  let mut marks: Vec<(usize, usize, bool)> = Vec::new();
  for (index, hit) in hits.iter().enumerate() {
    let Some(row) = row_of(hit) else { continue };
    let slot = row * TRACK_MARKS / rows.max(1);
    let is_current = current == Some(index);
    match marks.iter_mut().find(|(at, ..)| *at == slot) {
      Some(mark) => mark.2 |= is_current,
      None => marks.push((slot, index, is_current)),
    }
  }
  marks.sort_by_key(|(slot, ..)| *slot);
  // The room between marks is shared out by how far apart they sit, after the marks took theirs.
  let room = |slots: usize| {
    Column::new()
      .width(Dimension::Px(TRACK_WIDTH))
      .flex(f32::from(u16::try_from(slots).unwrap_or(u16::MAX)))
      .into()
  };
  let mut children: Vec<Element> = Vec::new();
  let mut above = 0;
  for (slot, first, is_current) in marks {
    children.push(room(slot - above));
    let height = if is_current { CURRENT_MARK_HEIGHT } else { MARK_HEIGHT };
    children.push(
      pressable(go(first))
        .id(format!("conversation-search-track-{first}"))
        .describe("current", is_current.to_string())
        .width(Dimension::Px(TRACK_WIDTH))
        .height(Dimension::Px(height))
        .flex_shrink(0.0)
        .rounded(ConversationRadius::TrackMark.role())
        .background(PaletteColor::Warning)
        .opacity(if is_current { 1.0 } else { TRACK_MARK_OPACITY })
        .into(),
    );
    above = slot;
  }
  children.push(room(TRACK_MARKS - above));
  Column::new()
    .id("conversation-search-track")
    .width(Dimension::Px(TRACK_WIDTH))
    .height(Dimension::Pct(100.0))
    .background(ExtraColor::Hover.role())
    .rounded(ExtraRadius::Full.role())
    .with_children(children)
    .into()
}
