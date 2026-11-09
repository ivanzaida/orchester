//! The panel of a tool, or of a profile that has only a draft, in a local project (`Detail` in L1;
//! `docs/design/team-configuration.md` rule 11): the head with its … menu, the chips (the version, how it stands), and
//! *Ready?*. The team hub's panel of a resource without a sheet is the same frame
//! ([`crate::screens::team_hub::panel`]).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use super::{
  archive,
  model::Entry,
  panel::{chips_of, frame, readiness, standing_chips},
  words::kind_where_key,
};
use crate::{
  project::ShownProject,
  screens::team_hub::panel::{body, head_named},
};

/// The panel of `entry`.
pub(super) fn panel(ctx: &mut Ctx, project: &ShownProject, entry: &Entry) -> Element {
  let more = archive::menu(
    ctx,
    archive::Target {
      root: project.open.repo_root.clone(),
      project: project.open.project.id(),
      what: entry.what,
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  let version = entry.number.map(|number| number.to_string());
  let body = body()
    .child(head_named(
      &entry.name,
      (ctx.t(kind_where_key(entry.kind)).to_string(), version),
      Some(more),
    ))
    .child(chips_of(standing_chips(ctx, entry), None))
    .with_children(readiness(ctx, entry));
  frame(entry.what, body, None)
}
