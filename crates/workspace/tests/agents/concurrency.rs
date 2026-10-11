//! The gap between an editor's validation and its transactional save/rebase.
use orchester_core::{id::ResourceId, resource::DraftSaved};
use orchester_workspace::agents::data::{self, ProfileEntry};

use super::*;

fn fixture(stale: bool) -> (TempDir, ProjectDb, ProfileEntry, AgentProfile) {
  let (repo, mut db) = project_db();
  let connection = codex(&repo);
  db.create_connection(&connection).expect("connection");
  let selection = ModelSelection::Listed(model("gpt-5.5"));
  let original = form(&connection, &selection, None).profile().expect("profile");
  let changed = form(&connection, &selection, Some("high")).profile().expect("edited");
  let id = create_profile(&mut db, &original, now()).expect("create");
  publish(&mut db, id, now()).expect("v1");
  if stale {
    save_draft(&mut db, id, changed.clone(), now()).expect("draft");
    publish(&mut db, id, now()).expect("v2");
    let stale = form(&connection, &selection, Some("low"))
      .profile()
      .expect("stale candidate");
    db.save_draft(id, &Draft::new(stale, Some(version(1)), now()))
      .expect("stale draft");
  }
  let expected = ProjectAgents::load(&db)
    .expect("snapshot")
    .profile(id)
    .expect("profile")
    .clone();
  (repo, db, expected, changed)
}

fn entry(db: &ProjectDb, id: ResourceId) -> ProfileEntry {
  ProjectAgents::load(db)
    .expect("snapshot")
    .profile(id)
    .expect("profile")
    .clone()
}

#[test]
fn profile_checked_save_rejects_intervening_body_base_and_version_changes() {
  for change in ["body", "base", "version", "removed"] {
    let (repo, mut db, published, candidate) = fixture(false);
    save_draft(&mut db, published.id, candidate.clone(), now()).expect("initial draft");
    let expected = entry(&db, published.id);
    // Another connection commits after the editor validated this snapshot.
    let mut other = ProjectDb::open(repo.path()).expect("other connection");
    match change {
      "body" => data::apply_in(
        repo.path(),
        data::ProfileChange::SaveDraft(
          expected.id,
          candidate.clone().with_permissions(AgentPermissions::new(
            orchester_core::permission::RepositoryAccess::Write,
            CommandPermission::AskBefore,
            NetworkAccess::On,
          )),
        ),
      )
      .map(|_| ())
      .expect("domain save"),
      "base" => {
        other
          .save_draft(expected.id, &Draft::new(candidate.clone(), None, now()))
          .expect("base change");
      }
      "version" => {
        publish(&mut other, expected.id, now()).expect("v2");
        other
          .save_draft(expected.id, &Draft::new(candidate.clone(), Some(version(1)), now()))
          .expect("same draft");
      }
      "removed" => {
        publish(&mut other, expected.id, now()).expect("published draft");
      }
      _ => unreachable!(),
    }
    let before = entry(&db, expected.id);
    let result = data::save_draft_checked_in(repo.path(), &expected, candidate);
    assert!(
      matches!(result, Err(StoreError::DraftChanged(_))),
      "{change}: {result:?}"
    );
    assert_eq!(entry(&db, expected.id), before, "{change}");
  }
}

#[test]
fn profile_checked_stale_publish_rejects_intervening_body_base_and_version_changes() {
  for change in ["body", "base", "version", "removed"] {
    let (repo, db, expected, candidate) = fixture(true);
    let mut other = ProjectDb::open(repo.path()).expect("other connection");
    match change {
      "body" => data::apply_in(
        repo.path(),
        data::ProfileChange::SaveDraft(expected.id, candidate.clone()),
      )
      .map(|_| ())
      .expect("domain save"),
      "base" => {
        other
          .save_draft(expected.id, &Draft::new(expected.profile.clone(), None, now()))
          .expect("base change");
      }
      "version" | "removed" => {
        keep_draft_over_latest(&mut other, expected.id, now()).expect("rebase");
        publish(&mut other, expected.id, now()).expect("v3");
        if change == "version" {
          other
            .save_draft(
              expected.id,
              &Draft::new(expected.profile.clone(), Some(version(1)), now()),
            )
            .expect("same draft");
        }
      }
      _ => unreachable!(),
    }
    let before = entry(&db, expected.id);
    let publish_result = data::publish_stale_checked_in(repo.path(), &expected);
    assert!(
      matches!(
        publish_result,
        Err(StoreError::DraftChanged(_) | StoreError::NoDraft(_))
      ),
      "{change}: {publish_result:?}"
    );
    assert_eq!(
      entry(&db, expected.id),
      before,
      "rejected publish changed storage: {change}"
    );
  }
}

#[test]
fn profile_checked_save_keeps_lifecycle_and_discards_repeated_drafts() {
  let (repo, db, expected, candidate) = fixture(false);
  data::save_draft_checked_in(repo.path(), &expected, candidate).expect("save");
  let saved = entry(&db, expected.id);
  assert_eq!(
    saved.state,
    VersionState::Draft {
      published: Some(version(1))
    }
  );
  data::save_draft_checked_in(repo.path(), &saved, expected.profile.clone()).expect("revert");
  assert_eq!(entry(&db, expected.id), expected);
}

#[test]
fn profile_draft_transaction_holds_write_lock_through_validation_and_persistence() {
  let (repo, mut db, expected, candidate) = fixture(false);
  let other = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("other connection");
  other.busy_timeout(std::time::Duration::ZERO).expect("no waiting");
  let saved = db
    .update_draft(expected.id, |draft, latest| {
      assert_eq!(
        ProfileEntry::new(expected.id, draft, latest.as_ref()),
        Some(expected.clone())
      );
      // An UPDATE between validation and persistence must fail, even from another SQLite connection.
      let attempt = other.execute("UPDATE resource_drafts SET based_on = NULL", []);
      assert!(matches!(attempt, Err(rusqlite::Error::SqliteFailure(error, _))
      if error.code == rusqlite::ErrorCode::DatabaseBusy));
      Ok(Draft::new(candidate.clone(), latest.map(|v| v.number()), now()))
    })
    .expect("atomic save");
  assert_eq!(saved, DraftSaved::Stored);
  assert_eq!(entry(&db, expected.id).profile, candidate);
  assert_eq!(
    other
      .execute("UPDATE resource_drafts SET based_on = NULL", [])
      .expect("unlocked"),
    1
  );
}
#[test]
fn profile_stale_publish_preserves_reviewed_body_and_holds_transaction_lock() {
  let (repo, mut db, expected, _) = fixture(true);
  let other = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("other");
  other.busy_timeout(std::time::Duration::ZERO).expect("no waiting");
  let published = db
    .publish_draft_with(expected.id, now(), |draft: Draft<AgentProfile>, latest| {
      assert_eq!(
        ProfileEntry::new(expected.id, Some(draft.clone()), latest.as_ref()),
        Some(expected.clone())
      );
      assert!(other.execute("UPDATE resource_drafts SET based_on = NULL", []).is_err());
      Ok(Draft::new(draft.body().clone(), latest.map(|v| v.number()), now()))
    })
    .expect("atomic publish");
  assert_eq!(published.body(), &expected.profile);
  assert_eq!(published.number(), version(3));
  assert!(db.draft::<AgentProfile>(expected.id).expect("draft").is_none());
}
