//! The project's daily reports in the Orchestrator chat (ORC-47): each day's kept once, whichever conversation keeps
//! it, read back as a report and by its day's key; one that starts a turn by itself gets it once, only while no turn
//! of its conversation thinks, and the runner reads the report's prompt; and the migration that lets the
//! conversation's entries hold reports keeps the entries it had.
//! `cargo test -p orchester-store --test orchestrator_reports reports`.

mod support;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, DailyReport, EntryKind, TurnState},
};
use orchester_store::ProjectDb;
use serde_json::json;
use support::{at, copied_at_version, model, started_run};

/// A report of 2026-10-20 that tells nothing but its tokens, starting a turn by itself when `automatic`.
fn report(automatic: bool) -> DailyReport {
  serde_json::from_value(json!({
    "day": "2026-10-20",
    "built": 1_000,
    "merged": [],
    "ready": [],
    "stopped": [],
    "working": [],
    "waiting": [],
    "actions": [],
    "usage": { "spent": 4_000, "active_ms": 60_000, "budget": 20_000_000, "tasks": [], "accounts": [], "link": "/p" },
    "queued": [],
    "automatic": automatic,
  }))
  .expect("a report")
}

fn key() -> String {
  DailyReport::key_of(report(false).day)
}

#[test]
fn reports_are_kept_once_a_day_and_read_back() {
  let (_repo, mut db, _run, _execution) = started_run();
  let first = Chat::named(ChatId::generate(), "Daily report", at(10));
  let second = Chat::named(ChatId::generate(), "Another", at(11));
  assert_eq!(db.daily_report(&key()).expect("read"), None);
  assert!(db.keep_daily_report(&first, &report(false), at(12)).expect("kept"));
  assert!(
    !db
      .keep_daily_report(&second, &report(false), at(13))
      .expect("not again"),
    "one report per day, whichever conversation"
  );
  assert_eq!(db.daily_report(&key()).expect("read"), Some(report(false)));
  let entries = db.chat_entries(first.id()).expect("entries");
  assert!(
    matches!(&entries[..], [entry] if matches!(&entry.kind, EntryKind::Report { report: kept, turn: None } if **kept == report(false))),
    "{entries:?}"
  );
  assert!(
    db.automatic_reports_without_turn().expect("reports").is_empty(),
    "it starts no turn"
  );
}

#[test]
fn reports_that_start_a_turn_get_it_once_with_the_reports_prompt() {
  let (_repo, mut db, _run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "Plan", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("How is ORC-1?").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.keep_daily_report(&chat, &report(true), at(11)).expect("kept");
  let [kept] = db
    .automatic_reports_without_turn()
    .expect("reports")
    .try_into()
    .expect("one");
  assert_eq!(kept.chat, chat.id());
  let turn = ChatMessageId::generate();
  assert!(
    !db.start_wait_turn((chat.id(), kept.seq), turn).expect("refused"),
    "the person's turn thinks"
  );
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  assert!(db.start_wait_turn((chat.id(), kept.seq), turn).expect("started"));
  assert!(db.automatic_reports_without_turn().expect("reports").is_empty());
  let prompt = db.chat_turn(turn).expect("read").expect("the turn").text;
  assert!(
    prompt
      .as_str()
      .starts_with("[Orchester] The project's daily report for 2026-10-20"),
    "{}",
    prompt.as_str()
  );
}

fn schema_version(path: &std::path::Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

#[test]
fn reports_migration_keeps_the_conversations_entries() {
  let (repo, mut db, _run, _execution) = started_run();
  let chat = Chat::named(ChatId::generate(), "Plan", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Plan ORC-1").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.add_chat_reply(chat.id(), ("Planned.", &model()), at(11))
    .expect("reply");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest = schema_version(&current);
  let old = copied_at_version(&current, latest - 1);
  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(schema_version(&old.path().join(".orchester/orchester.db")), latest);
  assert_eq!(upgraded.chat_entries(chat.id()).expect("entries").len(), 2);
  assert!(
    upgraded
      .keep_daily_report(&chat, &report(false), at(20))
      .expect("a report after the migration")
  );
  assert_eq!(upgraded.chat_entries(chat.id()).expect("entries").len(), 3);
}
