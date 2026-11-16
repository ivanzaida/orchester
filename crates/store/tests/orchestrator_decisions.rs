//! OA-22: the decisions the Orchestrator made on its own, kept in the project database: read open ones first and
//! then the newest first, by task, conversation and time, and reviewed forward only.
//! `cargo test -p orchester-store --test orchestrator_decisions decisions`.

mod support;

use std::fs;

use orchester_core::{
  id::{ChatId, ProjectId, RunId},
  orchestrator::decision::{NewDecision, Reason, Review, Undo},
  project::{Project, ProjectKey},
};
use orchester_store::{DecisionsRead, ProjectDb};
use support::{at, name};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn made(chat: ChatId, task: &str, when: i64) -> NewDecision {
  NewDecision {
    chat,
    tool: "continue_run".to_owned(),
    task: Some(task.to_owned()),
    run: Some(RunId::generate()),
    reason: Reason::new("The review's only comment is about wording.").expect("a reason"),
    undo: Some(Undo::ReturnToStep),
    at: at(when),
  }
}

fn numbers(db: &ProjectDb, read: &DecisionsRead) -> Vec<i64> {
  let decisions = db.decisions(read).expect("read");
  decisions.iter().map(|decision| decision.number).collect()
}

#[test]
fn decisions_are_kept_as_made_and_read_open_first_then_newest_first() {
  let (_repo, mut db) = project();
  let chat = ChatId::generate();
  let first = db.keep_decision(&made(chat, "ORC-1", 100)).expect("kept");
  let second = db.keep_decision(&made(chat, "ORC-2", 200)).expect("kept");
  let third = db.keep_decision(&made(chat, "ORC-3", 300)).expect("kept");

  let all = db.decisions(&DecisionsRead::default()).expect("read");
  assert_eq!(
    all[2].made,
    made_like(&all[2].made, chat, "ORC-1", 100),
    "kept as it was made"
  );
  assert!(all.iter().all(|decision| decision.review == Review::Open));
  assert_eq!(numbers(&db, &DecisionsRead::default()), vec![third, second, first]);

  // The person saw the newest: it goes after the open ones.
  assert!(
    db.review_decision(third, Review::Seen { at: at(400) })
      .expect("reviewed")
  );
  assert_eq!(numbers(&db, &DecisionsRead::default()), vec![second, first, third]);
  let open = DecisionsRead {
    open_only: true,
    ..DecisionsRead::default()
  };
  assert_eq!(numbers(&db, &open), vec![second, first]);
}

/// `kept` with only its run kept, which each decision made here has its own of.
fn made_like(kept: &NewDecision, chat: ChatId, task: &str, when: i64) -> NewDecision {
  NewDecision {
    run: kept.run,
    ..made(chat, task, when)
  }
}

#[test]
fn decisions_are_read_by_task_conversation_and_time() {
  let (_repo, mut db) = project();
  let (here, elsewhere) = (ChatId::generate(), ChatId::generate());
  let first = db.keep_decision(&made(here, "ORC-1", 100)).expect("kept");
  let second = db.keep_decision(&made(elsewhere, "ORC-1", 200)).expect("kept");
  let third = db.keep_decision(&made(here, "ORC-2", 300)).expect("kept");

  let of_task = DecisionsRead {
    task: Some("orc-1".to_owned()),
    ..DecisionsRead::default()
  };
  assert_eq!(numbers(&db, &of_task), vec![second, first], "a key is read in any case");
  let of_chat = DecisionsRead {
    chat: Some(here),
    ..DecisionsRead::default()
  };
  assert_eq!(numbers(&db, &of_chat), vec![third, first]);
  let of_day = DecisionsRead {
    since: Some(at(200)),
    until: Some(at(300)),
    ..DecisionsRead::default()
  };
  assert_eq!(numbers(&db, &of_day), vec![second], "from its start, before its end");
}

#[test]
fn decisions_are_reviewed_forward_only() {
  let (_repo, mut db) = project();
  let number = db.keep_decision(&made(ChatId::generate(), "ORC-1", 100)).expect("kept");
  let review = |db: &ProjectDb| db.decisions(&DecisionsRead::default()).expect("read")[0].review;

  assert!(
    !db
      .review_decision(number + 1, Review::Seen { at: at(150) })
      .expect("read"),
    "no such decision"
  );
  assert!(
    db.review_decision(number, Review::Seen { at: at(200) })
      .expect("reviewed")
  );
  assert_eq!(review(&db), Review::Seen { at: at(200) });
  assert!(
    !db.review_decision(number, Review::Seen { at: at(250) }).expect("read"),
    "seen once"
  );
  assert!(
    db.review_decision(number, Review::Overruled { at: at(300) })
      .expect("reviewed")
  );
  assert_eq!(review(&db), Review::Overruled { at: at(300) });
  assert!(
    !db.review_decision(number, Review::Seen { at: at(350) }).expect("read"),
    "not taken back"
  );
  assert_eq!(review(&db), Review::Overruled { at: at(300) });
}
