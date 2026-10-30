//! `shared-config` and `shared-config-member`: the `connections` catalogue as the team's configuration of a project
//! shared on `Acme team` at `orchester.acme.dev/kontur` (`Shared configuration · redesign` `UC5xk`). Every resource is
//! the team's, each version published by dana-k (the owner) or olha (a member). In `shared-config` the person is
//! dana-k: Builder has her draft on its latest version (02 `Sflw7`), Planner a version published here that waits for
//! the server (02b `iLwm9`), and Reviewer a version that met v2, which olha published from the same base (02c
//! `JZFlA`). In `shared-config-member` the person is ivan, a member, who reads them all (01b `P8tXS`, 03 `pjEjf`). No
//! server answers: the server is in Settings › Orchester, signed in, with no token in any keychain.

use std::collections::HashMap;

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ServerId, UserId},
  project::ProjectKey,
  resource::{
    AgentProfile, Draft, Instruction, Pipeline, PromptText, Removal, RemoveMode, ResourceBody, ResourceKind, Skill,
    Tool, VersionNumber,
  },
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
};
use orchester_store::{AppDb, CachedVersion, ServerAccount, SharedMember, SharingLink, StoredServer};

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

/// As dana-k, the project's owner: her draft, a version waiting for the server and one that met olha's.
pub fn admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  let team = shared(kontur, MemberRole::Owner)?;
  admin_states(kontur, &team)
}

/// As ivan, a member, who reads every resource of the team.
pub fn member(kontur: &mut Kontur) -> Result<(), SeedError> {
  shared(kontur, MemberRole::Member).map(|_| ())
}

/// The catalogue, linked to the team server's project with the person in `role`, every resource the team's. A member's
/// computer has no drafts of the team's: the catalogue's are dropped, and a resource never published with them.
fn shared(kontur: &mut Kontur, role: MemberRole) -> Result<Team, SeedError> {
  connections::seed(kontur)?;
  if role == MemberRole::Member {
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
  let me = if role == MemberRole::Owner {
    &team.dana
  } else {
    &team.ivan
  };
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
  share::<AgentProfile>(kontur, &team)?;
  share::<Pipeline>(kontur, &team)?;
  share::<Instruction>(kontur, &team)?;
  share::<Skill>(kontur, &team)?;
  share::<Tool>(kontur, &team)?;
  Ok(team)
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
  for (index, resource) in kontur.db.resources(B::KIND)?.into_iter().enumerate() {
    kontur.db.keep_shared_resource(&resource, revision)?;
    let publisher = if index % 2 == 0 { &team.dana } else { &team.olha };
    for version in kontur.db.versions::<B>(resource.id())? {
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

/// dana-k's draft on Builder, Planner published here and waiting, and Reviewer's version that met olha's v2.
fn admin_states(kontur: &mut Kontur, team: &Team) -> Result<(), SeedError> {
  for resource in kontur.db.resources(ResourceKind::AgentProfile)? {
    let Some(latest) = kontur.db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let id = resource.id();
    let edited = |text: &str| -> Result<AgentProfile, SeedError> {
      let body = latest.body();
      Ok(
        AgentProfile::new(
          body.role().clone(),
          body.responsibilities().to_owned(),
          PromptText::new(text).map_err(invalid)?,
          body.connection(),
          body.model().clone(),
          body.settings().clone(),
        )
        .with_permissions(body.permissions().clone())
        .with_mcp_servers(body.mcp_servers().clone()),
      )
    };
    match latest.body().role().as_str() {
      "Builder" => {
        let draft = Draft::new(
          edited("Work only inside the task worktree. Keep each commit small.")?,
          Some(latest.number()),
          kontur.today_ago(20),
        );
        kontur.db.save_draft(id, &draft)?;
      }
      "Planner" => {
        let draft = Draft::new(
          edited("Plan the change in small steps.")?,
          Some(latest.number()),
          kontur.today_ago(40),
        );
        kontur.db.save_draft(id, &draft)?;
        kontur.db.publish_draft::<AgentProfile>(id, kontur.today_ago(30))?;
      }
      "Reviewer" => {
        let draft = Draft::new(
          edited("Check the change against every criterion.")?,
          Some(latest.number()),
          kontur.today_ago(50),
        );
        kontur.db.save_draft(id, &draft)?;
        let mine = kontur.db.publish_draft::<AgentProfile>(id, kontur.today_ago(45))?;
        let next = VersionNumber::new(latest.number().get().saturating_add(1).try_into().map_err(invalid)?);
        let theirs = cached(
          edited("Review the diff, then the criteria.")?,
          next,
          &team.olha,
          kontur.today_ago(48),
        )?;
        kontur.db.cache_shared_version(id, &theirs)?;
        kontur.db.record_publish_conflict(mine.id(), next)?;
      }
      _ => {}
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
