//! Meter (`Meter` `wtAQI`): how much of something is used, as a 4 high `border` track with the used part
//! filled from its start, both radius 2. The fill is `text_secondary`; a limit that was reached fills it in
//! `warning` (`Meter · tokens` in `Raise the limit` `xHs82`); a download under way in `info` (`Progress` `vDmIX` in
//! Settings › Updates `c69Eti`). A context window's share (`Bar` `buxXQ` in the context popover `v6vdj`) fills in
//! `text_muted`, `warning` near full and `danger` once full. It is as wide as its parent.

use lurq::{
  node::Element,
  ui::{Progress, Styled},
};

use crate::theme::kit::meter as variant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeterTone {
  /// `text_secondary`: a share of something.
  Plain,
  /// `warning`: a limit that was reached.
  Reached,
  /// `info`: how far a download has come.
  Progress,
  /// `text_muted`: a context window with plenty of room (`nGKEB`).
  Quiet,
  /// `danger`: a context window that is full (`SmiLo`).
  Full,
}

impl MeterTone {
  const fn variant(self) -> &'static str {
    match self {
      Self::Plain => variant::PLAIN,
      Self::Reached => variant::REACHED,
      Self::Progress => variant::PROGRESS,
      Self::Quiet => variant::QUIET,
      Self::Full => variant::FULL,
    }
  }
}

/// A meter of `used` out of `of`, with the element id `id` (its fill `<id>-fill`): lurq's `ui::Progress` in the
/// theme's recipe (`theme::kit::meter`), which says the share as `share` to tooling. More than `of` fills the whole
/// track, and so does an `of` of nothing.
pub fn meter(id: &str, (used, of): (u64, u64), tone: MeterTone) -> Element {
  // A share of a few hundred pixels: f64 of a count is precise enough.
  let (used, of) = if of == 0 { (1.0, 1.0) } else { (used as f64, of as f64) };
  let percent = (used / of * 100.0).min(100.0);
  Progress::new(used)
    .max(of)
    .variant(tone.variant())
    .describe("share", format!("{percent:.0}"))
    .id(id.to_owned())
    .into()
}
