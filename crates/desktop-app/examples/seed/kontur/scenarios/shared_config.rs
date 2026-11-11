//! `shared-config` and `shared-config-member`: the `connections` catalogue as the team's configuration of a project
//! shared on `Acme team` at `orchester.acme.dev/kontur` (`Shared configuration · redesign` `UC5xk`). Every resource is
//! the team's, each version published by dana-k (the owner) or olha (a member and the server's admin). In
//! `shared-config` the person is dana-k, an admin; in `shared-config-member` ivan, a member, who reads them all (01b
//! `P8tXS`, 03 `pjEjf`). Nothing of the team's waits on this computer: a change is sent to the team server at once
//! (`docs/design/team-configuration-live.md`). No server answers: the server is in Settings › Orchester, signed in,
//! with no token in any keychain.

use std::collections::HashMap;

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ServerId, UserId},
  project::ProjectKey,
  resource::{AgentProfile, Instruction, Pipeline, Removal, RemoveMode, ResourceBody, Skill, Tool, VersionNumber},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{AppDb, CachedVersion, ServerAccount, ServerAdmin, SharedMember, SharingLink, StoredServer};

use super::connections;
use crate::kontur::{Kontur, SeedError};

const DAY: i64 = 24 * 60;
/// A hash the cached versions carry; the seed never compares them.
const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

/// The people of the team, by their username.
struct Team {
  dana: SharedPerson,
  olha: SharedPerson,
  ivan: SharedPerson,
}

/// As dana-k, the project's owner.
pub fn admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  let team = shared(kontur, (MemberRole::Owner, Person::Dana))?;
  share_catalogue(kontur, &team)
}

/// As dana-k, the project's owner, on a team that holds none of the catalogue: its resources stay this computer's, so
/// the team holds only what the caller adds (`team-hub`, whose hub lists the team's own resources alone).
pub fn admin_alone(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared(kontur, (MemberRole::Owner, Person::Dana)).map(|_| ())
}

/// As ivan, a member, who reads every resource of the team.
pub fn member(kontur: &mut Kontur) -> Result<(), SeedError> {
  let team = shared(kontur, (MemberRole::Member, Person::Ivan))?;
  share_catalogue(kontur, &team)
}

/// As ivan, a member, over what `resources` makes of the team's configuration, which holds none of the catalogue (as
/// [`admin_alone`]): a member cannot make a team resource, so it is made while the project is still linked in the
/// owner's role, which is dropped for ivan's afterwards.
pub fn member_over(
  kontur: &mut Kontur,
  resources: impl FnOnce(&mut Kontur) -> Result<(), SeedError>,
) -> Result<(), SeedError> {
  connections::seed(kontur)?;
  member_over_project(kontur, resources)
}

/// [`member_over`] on the project as another scenario already seeded it, with its own catalogue and connections.
pub fn member_over_project(
  kontur: &mut Kontur,
  resources: impl FnOnce(&mut Kontur) -> Result<(), SeedError>,
) -> Result<(), SeedError> {
  linked(kontur, (MemberRole::Owner, Person::Ivan))?;
  resources(kontur)?;
  kontur.db.record_sharing_state(Some(MemberRole::Member), None, None)?;
  Ok(())
}

/// Who the computer's person is on the team server.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Person {
  Dana,
  Ivan,
}

/// The catalogue, linked to the team server's project with the person in `role`, none of it the team's yet. Ivan's
/// computer has no drafts of the team's: the catalogue's are dropped, and a resource never published with them.
fn shared(kontur: &mut Kontur, (role, who): (MemberRole, Person)) -> Result<Team, SeedError> {
  connections::seed(kontur)?;
  linked(kontur, (role, who))
}

/// The project as it is, linked to the team server's project with the person in `role`.
fn linked(kontur: &mut Kontur, (role, who): (MemberRole, Person)) -> Result<Team, SeedError> {
  if who == Person::Ivan {
    drop_drafts::<AgentProfile>(kontur)?;
    drop_drafts::<Pipeline>(kontur)?;
    drop_drafts::<Instruction>(kontur)?;
    drop_drafts::<Skill>(kontur)?;
    drop_drafts::<Tool>(kontur)?;
  }
  let team = Team {
    dana: person("dana-k"),
    olha: person("olha"),
    ivan: person("ivan"),
  };
  let me = if who == Person::Dana { &team.dana } else { &team.ivan };
  let server = server(kontur, me)?;
  let link = SharingLink {
    server,
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").map_err(invalid)?,
    role: Some(role),
    joined_at: kontur.ago(30 * DAY),
    cursor: 0,
    last_synced_at: kontur.ago(5),
    removed_at: None,
    address: Some("https://orchester.acme.dev/kontur".to_owned()),
  };
  kontur.db.start_sharing(&link, &[], (&[], &HashMap::new()))?;
  let added = kontur.ago(30 * DAY);
  let members = [
    (&team.dana, "Dana Kovalenko", MemberRole::Owner),
    (&team.olha, "Olha", MemberRole::Member),
    (&team.ivan, "Ivan", MemberRole::Member),
  ]
  .map(|(who, display, role)| SharedMember {
    user: who.id,
    display_name: display.to_owned(),
    username: who.name.clone(),
    role,
    added_at: added,
  });
  kontur.db.record_shared_members(&members, kontur.ago(5), Some(me.id))?;
  let admin = ServerAdmin {
    user: team.olha.id,
    display_name: "Olha".to_owned(),
    username: team.olha.name.clone(),
  };
  kontur.db.record_server_admins(&[admin])?;
  Ok(team)
}

/// Makes every published resource of the catalogue the team's.
fn share_catalogue(kontur: &mut Kontur, team: &Team) -> Result<(), SeedError> {
  share::<AgentProfile>(kontur, team)?;
  share::<Pipeline>(kontur, team)?;
  share::<Instruction>(kontur, team)?;
  share::<Skill>(kontur, team)?;
  share::<Tool>(kontur, team)
}

/// `Acme team` in Settings › Orchester, signed in as `me` (dana-k is not the server's admin: the owner changes the
/// configuration by her role in the project).
fn server(kontur: &mut Kontur, me: &SharedPerson) -> Result<ServerId, SeedError> {
  let id = ServerId::generate();
  let stored = StoredServer {
    id,
    url: "https://orchester.acme.dev".to_owned(),
    name: "Acme team".to_owned(),
    plain_http_allowed: false,
    certificate_pin: None,
    added_at: kontur.ago(30 * DAY),
    joined_with: None,
    account: Some(ServerAccount {
      user: me.id,
      username: me.name.clone(),
      display_name: me.name.clone(),
      admin: false,
      signed_in_at: kontur.ago(30 * DAY),
    }),
  };
  AppDb::open(&kontur.app_database)?.add_server(&stored, None)?;
  Ok(id)
}

/// Marks every resource of `B`'s kind the team's, each version as the server confirmed it, published by dana-k or olha
/// in turn.
fn share<B: ResourceBody + Clone>(kontur: &mut Kontur, team: &Team) -> Result<(), SeedError> {
  let revision = TaskRevision::try_from(1_u64).map_err(invalid)?;
  for (index, resource) in kontur.db.kept_resources(B::KIND)?.into_iter().enumerate() {
    let versions = kontur.db.versions::<B>(resource.id())?;
    // The server has only what was published: a draft of this computer's own stays local.
    if versions.is_empty() {
      continue;
    }
    kontur.db.keep_shared_resource(&resource, revision)?;
    let publisher = if index % 2 == 0 { &team.dana } else { &team.olha };
    for version in versions {
      let cached = cached(
        version.body().clone(),
        version.number(),
        publisher,
        version.published_at(),
      )?;
      kontur.db.cache_shared_version(resource.id(), &cached)?;
    }
  }
  Ok(())
}

/// Drops every draft of `B`'s kind: a published resource's goes, a resource never published goes with it.
fn drop_drafts<B: ResourceBody + PartialEq>(kontur: &mut Kontur) -> Result<(), SeedError> {
  for resource in kontur.db.resources(B::KIND)? {
    let Some(draft) = kontur.db.draft::<B>(resource.id())? else {
      continue;
    };
    if kontur.db.latest_version::<B>(resource.id())?.is_some() {
      kontur.db.discard_draft_matching(resource.id(), draft.body())?;
      continue;
    }
    let removal = kontur
      .db
      .remove_resource(resource.id(), RemoveMode::Unused, kontur.now)?;
    if let Removal::InUse(_) = removal {
      return Err(SeedError::Invalid(format!("{} is in use", resource.name().as_str())));
    }
  }
  Ok(())
}

/// `body` as the server confirmed it as version `number`, published by `publisher` at `at`, naming nothing.
fn cached<B: ResourceBody>(
  body: B,
  number: VersionNumber,
  publisher: &SharedPerson,
  at: orchester_core::time::Timestamp,
) -> Result<CachedVersion<B>, SeedError> {
  Ok(CachedVersion {
    number,
    published_at: at,
    publisher: publisher.clone(),
    server_body: serde_json::to_value(&body).map_err(invalid)?,
    hash: ContentHash::new(HASH).map_err(invalid)?,
    references: Vec::new(),
    local_body: body,
  })
}

fn person(username: &str) -> SharedPerson {
  SharedPerson {
    id: UserId::generate(),
    name: username.to_owned(),
  }
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}
