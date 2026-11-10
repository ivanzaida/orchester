//! What the sheets of the team's tools, instructions and skills share: who published the latest version, and which of
//! the team's profiles use a resource (`Agents · Tools` `u9Hcc` A6, `Agents · Instructions` `c3jU1R` A4,
//! `Agents · Skills` `c9YnL` A5).

use orchester_core::id::ResourceId;

use super::{team_body_edit::Latest, team_mcp::Published, team_profile::TeamProfileSheet};
use crate::team_config::{TeamConfig, TeamResource};

/// Who published the latest version of `resource` and when; `None` while it has none.
pub(super) fn published(resource: &TeamResource) -> Option<Published> {
  resource.latest.as_ref().map(|latest| Published {
    by: latest.published_by.clone(),
    at: latest.published_at,
  })
}

/// The names of the team's profiles whose sheet `uses` the resource.
pub(super) fn used_by(
  config: &TeamConfig,
  profiles: &[(ResourceId, TeamProfileSheet)],
  uses: impl Fn(&TeamProfileSheet) -> bool,
) -> Vec<String> {
  profiles
    .iter()
    .filter(|(_, sheet)| uses(sheet))
    .filter_map(|(profile, _)| config.name(*profile).map(str::to_owned))
    .collect()
}

/// What an admin's edit of `resource` starts from: its latest version, whose body this computer read as `body`.
pub(super) fn latest_of<B: Clone>(resource: &TeamResource, body: &B) -> Option<Latest<B>> {
  Some(Latest {
    number: resource.number()?,
    body: body.clone(),
  })
}
