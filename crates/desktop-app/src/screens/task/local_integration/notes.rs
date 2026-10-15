//! What the page says around its evidence. A panel is a heading in 14 `text_primary` over what it explains in
//! 11.5 `text_secondary`, 8 apart and 16 inside a radius 10 stroke: `warning` where the step waits for the person
//! (`Dirty checkout notice` `t6PHFE`, `Remaining blocker` `pshey`), `border` where it says what a review does
//! (`Commit outcome preview` `uOWTu`, `Preservation intent` `MHicq`, `Recovery boundaries` `BE2MH`). Other phases
//! keep the kit's notice in their tone.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::Element,
};
use orchester_core::integration::local::LocalPhase;

use super::{
  Mode,
  words::{phase_body, phase_title},
};
use crate::{
  theme::LocalText,
  ui::{
    card::{CardSurface, card_frame},
    notice::{Notice, NoticeTone},
  },
};

/// A panel `id` on `surface`: `title` over `body`.
pub(super) fn panel(id: &str, surface: CardSurface, title: &str, body: &str) -> Column {
  card_frame(id, surface)
    .padding(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(LocalText::Heading.role()),
    )
    .child(
      Text::new(body)
        .id(format!("{id}-body"))
        .variant(LocalText::Explanation.role()),
    )
}

/// What the page is about now, first on its evidence side.
pub(super) fn phase(ctx: &Ctx, phase: LocalPhase, mode: &Mode, narrow: bool) -> Element {
  let (surface, title, body) = match mode {
    Mode::Commit { .. } => (
      CardSurface::Outline,
      "local_integration.commitIntro",
      "local_integration.commitIntroBody",
    ),
    Mode::Preserve => (
      CardSurface::Outline,
      "local_integration.preserveIntro",
      "local_integration.preserveIntroBody",
    ),
    _ if phase == LocalPhase::Review && narrow => (
      CardSurface::Pending,
      "local_integration.dirtyNarrow",
      "local_integration.dirtyNarrowBody",
    ),
    _ if phase == LocalPhase::Review => (
      CardSurface::Pending,
      "local_integration.dirty",
      "local_integration.dirtyBody",
    ),
    _ => {
      return Notice::new("local-phase", tone(phase), ctx.t(phase_title(phase)))
        .body(ctx.t(phase_body(phase)))
        .into();
    }
  };
  panel("local-phase", surface, &ctx.t(title), &ctx.t(body)).into()
}

fn tone(phase: LocalPhase) -> NoticeTone {
  match phase {
    LocalPhase::Complete | LocalPhase::RestorationVerified => NoticeTone::Success,
    LocalPhase::TaskConflicts => NoticeTone::Danger,
    LocalPhase::Review
    | LocalPhase::RestoreConflicts
    | LocalPhase::MergedAwaitingRestore
    | LocalPhase::Paused
    | LocalPhase::NeedsReconciliation => NoticeTone::Warning,
    _ => NoticeTone::Info,
  }
}

/// Where ignored files stand, under the reviewed diff (`Ignored files boundary` `c1zep`).
pub(super) fn ignored(ctx: &Ctx, narrow: bool) -> Element {
  let key = if narrow {
    "local_integration.ignoredNarrow"
  } else {
    "local_integration.ignored"
  };
  Text::new(&ctx.t(key))
    .id("local-ignored")
    .variant(LocalText::Note.role())
    .into()
}

/// Under the preservation's selection: the snapshots it keeps because it keeps whole paths, none when it keeps
/// only the selected ones.
pub(super) fn kept_whole(ctx: &Ctx, kept: &[String]) -> Option<Element> {
  (!kept.is_empty()).then(|| {
    Text::new(&ctx.t_args(
      "local_integration.keptWhole",
      [("snapshots", &*kept.join(&ctx.t("local_integration.separator")))],
    ))
    .id("local-kept-whole")
    .variant(LocalText::Note.role())
    .into()
  })
}

/// What the selected commit leaves in the checkout (`Remaining blocker` `pshey`): the snapshots it excludes, or
/// that nothing remains.
pub(super) fn remaining(ctx: &Ctx, excluded: &[String]) -> Element {
  let body = if excluded.is_empty() {
    ctx.t("local_integration.remainingNone")
  } else {
    ctx.t_args(
      "local_integration.remainingBody",
      [("paths", &*excluded.join(&ctx.t("local_integration.separator")))],
    )
  };
  panel(
    "local-remaining",
    CardSurface::Pending,
    &ctx.t("local_integration.remaining"),
    &body,
  )
  .into()
}

/// That preservation discards nothing silently (`Recovery boundaries` `BE2MH`).
pub(super) fn no_discard(ctx: &Ctx) -> Element {
  panel(
    "local-no-discard",
    CardSurface::Outline,
    &ctx.t("local_integration.noDiscard"),
    &ctx.t("local_integration.noDiscardBody"),
  )
  .into()
}
