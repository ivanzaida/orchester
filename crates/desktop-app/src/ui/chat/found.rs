//! What a search of a chat found in a message's words (ORC-622, `Match` and `Match · current` in 13b `c0CQp` and
//! 13f `aoVlE` of `Task page · redesign` `C3wC8`): a fill behind each place in the words as the message shows them,
//! `warning_muted`, the current one `warning`, radius `radius_sm`, one fill per laid-out line, under the glyphs and
//! a selection (lurq's `TextHighlight`). A message searches the words it shows for the needle itself, so the fills
//! sit on its words however its Markdown renders; the current place is the one at the same rank among the message's
//! places as among those the search found in it. The words keep their color: lurq's fills do not recolor glyphs, so
//! the current place's words are not the design's `text_inverse` 500.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::TextHighlight,
  node::Element,
};
use orchester_workspace::tasks::timeline::find::Needle;

/// The class of a message's words that hold places the search found, for tests and QA.
pub const MARKED_CLASS: &str = "search-marked";
/// The class of a message's words that hold the search's current place.
pub const CURRENT_CLASS: &str = "search-current";

/// What a search marks in a message: what it looks for, and the rank of its current place among the message's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
  needle: Needle,
  current: Option<usize>,
}

impl Found {
  pub const fn new(needle: Needle, current: Option<usize>) -> Self {
    Self { needle, current }
  }

  /// What it marks in the part of a message after its first `places` places, as when the chat draws the message in
  /// parts (`super::fences`): the current place is at its rank among the part's places.
  #[must_use]
  pub fn after(&self, places: usize) -> Self {
    Self {
      needle: self.needle.clone(),
      current: self.current.and_then(|rank| rank.checked_sub(places)),
    }
  }

  /// How many places of the needle `text` holds.
  pub fn count(&self, text: &str) -> usize {
    self.needle.places(text).len()
  }

  /// The fills behind the places of the needle in `text`, the words a message shows.
  pub fn highlights(&self, ctx: &Ctx, text: &str) -> Vec<TextHighlight> {
    let radius = ctx.theme().radius_value(RadiusSize::Sm);
    self
      .needle
      .places(text)
      .into_iter()
      .enumerate()
      .map(|(rank, range)| {
        let fill = if self.current == Some(rank) {
          PaletteColor::Warning
        } else {
          PaletteColor::WarningMuted
        };
        TextHighlight::new(range, fill).radius(radius)
      })
      .collect()
  }

  /// `words`, the element that shows a message's words with `places` fills, with the classes that say so.
  pub fn classed(&self, words: Element, places: usize) -> Element {
    if places == 0 {
      return words;
    }
    let words = words.class(MARKED_CLASS);
    if self.current.is_some_and(|rank| rank < places) {
      words.class(CURRENT_CLASS)
    } else {
      words
    }
  }
}
