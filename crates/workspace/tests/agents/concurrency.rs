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
fn profile_checked_keep_rejects_intervening_body_base_and_version_changes() {
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
    let result = data::keep_draft_checked_in(repo.path(), &expected);
    assert!(
      matches!(result, Err(StoreError::DraftChanged(_))),
      "{change}: {result:?}"
    );
    assert_eq!(entry(&db, expected.id), before, "{change}");
  }
}

#[test]
fn profile_checked_edits_keep_lifecycle_and_discard_repeated_drafts() {
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

  let (repo, db, expected, _) = fixture(true);
  data::keep_draft_checked_in(repo.path(), &expected).expect("keep confirmed draft");
  let saved = entry(&db, expected.id);
  assert_eq!(saved.profile, expected.profile);
  assert_eq!(
    saved.state,
    VersionState::Draft {
      published: Some(version(2))
    }
  );
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
