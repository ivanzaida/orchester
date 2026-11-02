//! The Skills section's data (ORC-325): the project's published skills (`orchester_workspace::skills::offered`), read
//! off the UI thread while the menu is open, again when it opens and when the project's resources change; and what
//! makes them unavailable for the message the composer writes.

use std::{path::PathBuf, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_workspace::{
  background,
  messages::Message,
  skills::offered::{OfferedSkill, published_in},
};

/// The skills a composer offers: those of the project at `root`, read again when `revision` moves; `refused` says why
/// the agent the message goes to cannot take one, which greys every skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillsOffer {
  pub root: PathBuf,
  pub revision: u64,
  pub refused: Option<Arc<str>>,
}

/// The published skills as the menu reads them.
pub(super) enum Read {
  Reading,
  Ready(Vec<OfferedSkill>),
  Failed(Message),
}

/// The published skills of `offer`'s project, read while the menu is `open`.
pub(super) fn read(ctx: &mut Ctx, offer: &SkillsOffer, open: bool) -> Read {
  let state = ctx
    .future(
      (offer.root.clone(), offer.revision, open),
      |(root, _, open)| async move {
        if !open {
          return Ok(None);
        }
        background::run(move || published_in(&root)).await?.map(Some)
      },
    )
    .state()
    .get();
  let pending = state.is_pending();
  // A failed re-read keeps the last data; it must not stay choosable as if current.
  match (state.error, state.data.flatten()) {
    (Some(error), _) if !pending => Read::Failed(error),
    (None, Some(skills)) => Read::Ready(skills),
    _ => Read::Reading,
  }
}
