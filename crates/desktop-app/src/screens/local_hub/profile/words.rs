//! What the head and the chips of a local profile's panel say: its version and when it was published, how it stands,
//! and the pipelines that use it.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{resource::VersionNumber, time::Timestamp};
use orchester_workspace::{
  agents::local_hub::LocalProfile,
  readiness::{Group, ReadinessStanding},
};

use super::ready::Fix;
use crate::{
  screens::{
    local_hub::model::Entry,
    team_hub::{
      profile::parts::{Part, State},
      words::date,
    },
  },
  ui::chip::BadgeTone,
};

/// *v4 · published Oct 27*.
pub(super) fn version_line(ctx: &Ctx, profile: &LocalProfile, number: Option<VersionNumber>) -> String {
  match number {
    Some(number) => ctx
      .t_args(
        "local_hub.profile.version",
        [
          ("version", number.to_string()),
          ("date", date(ctx, Timestamp::now(), profile.published_at).to_string()),
        ],
      )
      .to_string(),
    None => ctx.t("local_hub.line.draft").to_string(),
  }
}

/// The chips: the version, then how the profile stands.
pub(super) fn chips(
  ctx: &Ctx,
  entry: &Entry,
  profile: &LocalProfile,
  parts: &[Part<Fix>],
) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = Vec::new();
  if let Some(number) = entry.number {
    chips.push((Arc::from(number.to_string()), BadgeTone::Neutral));
  }
  chips.push(match (entry.group, entry.standing.as_ref()) {
    (Group::NeedsYouHere, Some(standing)) => (stands(ctx, entry, profile, parts, standing), BadgeTone::Warning),
    _ => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
  });
  chips
}

/// What the person has to do first, short enough for a chip.
fn stands(
  ctx: &Ctx,
  entry: &Entry,
  profile: &LocalProfile,
  parts: &[Part<Fix>],
  standing: &ReadinessStanding,
) -> Arc<str> {
  let connection = parts
    .first()
    .is_some_and(|part| matches!(part.state, State::Missing { .. }));
  match (connection, profile.connections.len()) {
    (true, 0) => ctx.t("local_hub.chip.addConnection"),
    (true, _) => ctx.t("local_hub.chip.chooseConnection"),
    (false, _) => crate::screens::local_hub::words::needs(ctx, entry, standing),
  }
}

/// *used by Designer, Reviewer*: the pipelines whose steps run it.
pub(super) fn used_by(ctx: &Ctx, profile: &LocalProfile) -> Option<Arc<str>> {
  (!profile.pipelines.is_empty())
    .then(|| ctx.t_args("local_hub.panel.usedBy", [("names", profile.pipelines.join(", "))]))
}
