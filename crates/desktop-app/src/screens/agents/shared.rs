//! A shared profile's page on this computer (`Shared configuration · redesign` `UC5xk`): a teammate's version that
//! widens what agents may do here (`02d` `FuOXA`: what it adds, with *Use v<N> here*, which only the person presses),
//! and the page head's *Runs here use v4* while it waits. What the team's profile needs of this computer is read by
//! [`orchester_workspace::readiness`] and filled in from the Agents hub's panel.

mod adds;
mod use_here;

use std::path::PathBuf;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, VersionNumber},
  sharing::VersionWidening,
};
use orchester_store::ProjectDb;
use orchester_workspace::{background, sharing::sync};
use use_here::{UseHere, UseHereProps};

use crate::{
  screens::{
    hub::{parts::Tone as DotTone, team::Here as HeadHere},
    settings::orchester::{Said, follow_shared, sharing_said},
  },
  services::Services,
  ui::field::field_error,
};

/// What this computer has of a shared profile: a version that waits for the person.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shared {
  waiting: Option<Waiting>,
}

impl DevtoolsInspectable for Shared {}

/// A teammate's version that widens, who published it, what it adds, and the version runs use meanwhile.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Waiting {
  number: VersionNumber,
  used: Option<VersionNumber>,
  publisher: String,
  adds: Vec<VersionWidening>,
}

/// What the page shows of a shared profile.
#[derive(Default)]
pub(super) struct Parts {
  /// The bars under the page head.
  pub bars: Vec<Element>,
  /// What the page head says of this computer: *Runs here use v4* while a widening version waits.
  pub here: Option<HeadHere>,
}

/// What the page shows of it, read for the profile `props` shows.
pub(super) fn parts(ctx: &mut Ctx, props: &super::editor::Props) -> Parts {
  let Some(entry) = &props.entry else {
    return Parts::default();
  };
  let root = props.project.open.repo_root.clone();
  match read(ctx, root.clone(), entry.id, props.revision.get()) {
    Err(reason) => Parts {
      bars: vec![field_error("agent-shared-unread", &reason.text(ctx)).into()],
      ..Parts::default()
    },
    Ok(None) => Parts::default(),
    Ok(Some(shared)) => Parts {
      bars: bar(ctx, &shared, entry.id, (root, &props.revision))
        .into_iter()
        .collect(),
      here: head_here(ctx, &shared),
    },
  }
}

/// Reads it for profile `resource` of the project at `root`; anew when `revision` or a shared project's cache changes.
fn read(ctx: &mut Ctx, root: PathBuf, resource: ResourceId, revision: u64) -> Result<Option<Shared>, Said> {
  let servers = Services::of(ctx).servers;
  let shared = follow_shared(ctx, &servers);
  let state = ctx
    .future((root, resource, revision, shared), |(root, resource, ..)| async move {
      background::run(move || read_shared(&root, resource))
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map_err(|error| sharing_said(&error))
    })
    .state()
    .get();
  // A failed re-read keeps the last data; it must not stay answerable (Use v<N> here) as if current.
  match (state.data, state.error) {
    (_, Some(reason)) => Err(reason),
    (Some(shared), None) => Ok(Some(shared)),
    (None, None) => Ok(None),
  }
}

fn read_shared(
  root: &std::path::Path,
  resource: ResourceId,
) -> Result<Shared, orchester_workspace::sharing::SharingError> {
  let waiting = match sync::waiting(root, resource)? {
    Some(waiting) => {
      // A version waits only once it is cached from the server, with its publisher; a cache cleared meanwhile
      // leaves the bar without the name rather than without the version.
      let publisher = ProjectDb::open(root)?
        .cached_version::<AgentProfile>(resource, waiting.number)?
        .map(|cached| cached.publisher.name)
        .unwrap_or_default();
      Some(Waiting {
        number: waiting.number,
        used: waiting.used,
        publisher,
        adds: waiting.adds,
      })
    }
    None => None,
  };
  Ok(Shared { waiting })
}

/// The bar under the page head: the version that waits for *Use v<N> here*.
fn bar(
  ctx: &mut Ctx,
  shared: &Shared,
  resource: ResourceId,
  (root, revision): (PathBuf, &Signal<u64>),
) -> Option<Element> {
  let waiting = shared.waiting.as_ref()?;
  Some(ctx.mount_keyed::<UseHere>(
    &format!("agent-use-here-{resource}"),
    UseHereProps {
      root,
      resource,
      waiting: waiting.clone(),
      revision: revision.clone(),
    },
  ))
}

/// What the page head says of this computer: *Runs here use v4* while a widening version waits (02d).
fn head_here(ctx: &Ctx, shared: &Shared) -> Option<HeadHere> {
  let used = shared.waiting.as_ref()?.used?;
  Some(HeadHere {
    tone: DotTone::Warning,
    text: ctx.t_args("sharing.runsHere.usesVersion", [("version", used.get().to_string())]),
  })
}
