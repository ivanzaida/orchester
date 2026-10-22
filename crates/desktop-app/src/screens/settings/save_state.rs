//! What Settings' header says of the changes on its open tab (`Save state` `uTF2N` in `oD7hy`, `d5Ig1H` in `hUaXB`):
//! each change saves as it is made, so the header says *All changes saved* unless one is being saved (*Saving…*) or
//! was not saved (*A change was not saved*, its row says why). The tab's rows report what each of their saves did
//! while they render; the header changes only when what it says does, and starts again on another tab.

use std::{collections::BTreeMap, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use parking_lot::Mutex;

use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// What the header says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Summary {
  #[default]
  Saved,
  Saving,
  Failed,
}

impl lurq::app::component::DevtoolsInspectable for Summary {}

/// What one save did, as its row reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
  Saved,
  Saving,
  Failed,
}

impl Outcome {
  /// What a save whose action is in `state` did: running, failed or not, or not run, which leaves nothing unsaved.
  pub fn of<T, E>(state: &FutureState<T, E>) -> Self {
    match (state.is_pending(), &state.error) {
      (true, _) => Self::Saving,
      (false, Some(_)) => Self::Failed,
      (false, None) => Self::Saved,
    }
  }
}

/// The saves of the open tab, shared by its rows and the header. Clones share them.
#[derive(Clone)]
pub struct SaveState {
  saves: Arc<Mutex<BTreeMap<&'static str, Outcome>>>,
  summary: Signal<Summary>,
}

impl PartialEq for SaveState {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.saves, &other.saves)
  }
}

impl lurq::app::component::DevtoolsInspectable for SaveState {}

impl SaveState {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      saves: Arc::default(),
      summary: ctx.signal(Summary::Saved),
    }
  }

  /// The tab's save state, which Settings provides; `None` outside it.
  pub fn of(ctx: &mut Ctx) -> Option<Self> {
    ctx.use_context::<Self>()
  }

  /// What the save of the row `key` did now.
  pub fn report(&self, key: &'static str, outcome: Outcome) {
    let summary = {
      let mut saves = self.saves.lock();
      if saves.get(key) == Some(&outcome) {
        return;
      }
      saves.insert(key, outcome);
      summarize(&saves)
    };
    if self.summary.get_untracked() != summary {
      self.summary.set(summary);
    }
  }

  /// Forgets the saves of a tab that is no longer shown.
  pub fn clear(&self) {
    self.saves.lock().clear();
    if self.summary.get_untracked() != Summary::Saved {
      self.summary.set(Summary::Saved);
    }
  }

  /// What the header says now; reading it follows its changes.
  pub fn summary(&self) -> Summary {
    self.summary.get()
  }
}

/// A save running comes first, then one that failed.
fn summarize(saves: &BTreeMap<&'static str, Outcome>) -> Summary {
  if saves.values().any(|outcome| *outcome == Outcome::Saving) {
    Summary::Saving
  } else if saves.values().any(|outcome| *outcome == Outcome::Failed) {
    Summary::Failed
  } else {
    Summary::Saved
  }
}

/// *✓ All changes saved* in 12 `text_muted` after a 13 icon, 6 apart (`d5Ig1H`).
pub fn label(ctx: &Ctx, summary: Summary) -> Element {
  let (glyph, key, tone) = match summary {
    Summary::Saved => (Icon::Check, "settings.saveState.saved", PaletteColor::TextMuted),
    Summary::Saving => (Icon::LoaderCircle, "settings.saveState.saving", PaletteColor::TextMuted),
    Summary::Failed => (Icon::CircleAlert, "settings.saveState.failed", PaletteColor::Danger),
  };
  Row::new()
    .id("settings-save-state")
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Snug.role())
    .describe(
      "state",
      match summary {
        Summary::Saved => "saved",
        Summary::Saving => "saving",
        Summary::Failed => "failed",
      },
    )
    .child(icon(glyph, IconSize::Base, tone.clone()))
    .child(
      Text::new(&ctx.t(key))
        .id("settings-save-state-text")
        .variant(TypographyStyle::Description)
        .color(tone)
        .nowrap(),
    )
    .into()
}
