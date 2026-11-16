//! How a local tool stands, as its panel says it (L21 `ldRQO`, L24 `OSniP`): the chips under the head (its version,
//! *Unsaved changes* while the fields differ from it, then *Needs a secret value*, what else it needs, or *Ready*), and
//! the project secrets it uses that have no value on this computer, which the secret's row says in place of a *Ready?*
//! section.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{id::SecretId, sharing::Missing};
use orchester_workspace::readiness::ReadinessStanding;

use crate::{
  screens::local_hub::{model::Entry, panel::standing_chips},
  ui::chip::BadgeTone,
};

/// The chips: its version, *Unsaved changes* when `changed`, then how it stands.
pub(super) fn chips(ctx: &Ctx, entry: &Entry, changed: bool) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = standing_chips(ctx, entry);
  let standing = chips.pop();
  if changed {
    chips.push((ctx.t("local_hub.chip.unsaved"), BadgeTone::Warning));
  }
  if valueless(entry).is_empty() {
    chips.extend(standing);
  } else {
    chips.push((ctx.t("local_hub.chip.needsSecret"), BadgeTone::Warning));
  }
  chips
}

/// The project secrets the tool uses that have no value on this computer.
pub(super) fn valueless(entry: &Entry) -> Vec<SecretId> {
  let Some(ReadinessStanding::Missing(missing)) = &entry.standing else {
    return Vec::new();
  };
  missing
    .missing()
    .iter()
    .filter_map(|part| match part {
      Missing::LocalSecret { secret, .. } => Some(*secret),
      _ => None,
    })
    .collect()
}
