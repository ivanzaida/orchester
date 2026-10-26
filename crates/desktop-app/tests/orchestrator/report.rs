//! The project's daily report in the Orchestrator conversation (ORC-47): its entry opens a row of its own, one line
//! with its day, its totals and when it was delivered, which opens into each part's heading and lines, every item a
//! link into the app, and what the Orchestrator does with it while *Let the Orchestrator answer agents* is on.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ChatId,
  orchestrator::{Chat, DailyReport, EntryKind},
  time::Timestamp,
  usage::local_date,
};
use orchester_workspace::orchestrator::report::build_in;

use super::harness::{Orchestrator, Setup};

/// Keeps today's report of `screen`'s project in a conversation of its own, starting a turn when `automatic`; the
/// report's entry.
fn keep_report(screen: &Orchestrator, automatic: bool) -> (DailyReport, i64) {
  let now = Timestamp::now();
  let built = build_in(&screen.root, Vec::new(), (local_date(now), now)).expect("built");
  let report = DailyReport { automatic, ..built };
  let chat = Chat::named(ChatId::generate(), "Daily report", now);
  let mut db = screen.db();
  assert!(db.keep_daily_report(&chat, &report, now).expect("kept"));
  let seq = db
    .chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .find_map(|entry| matches!(entry.kind, EntryKind::Report { .. }).then_some(entry.seq))
    .expect("the report's entry");
  (report, seq)
}

#[test]
fn orchestrator_report_shows_its_day_its_parts_and_their_links() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let key = screen.draft_on(screen.delivery(), "Read the readme");
  let (report, seq) = keep_report(&screen, false);
  screen.go(OrchestratorChat::Latest);
  let card = format!("orchestrator-report-wait-{seq}");
  screen.until("the report's line", |screen| screen.exists(&card));
  // As History says a date (*Oct 26*).
  let day = report.day.format("%b %-d").to_string();
  let line = screen.text(&card);
  assert!(
    line.contains(&format!("Daily report · {day}")) && line.contains("merged") && line.contains("tokens"),
    "one line with its day and totals: {line}"
  );
  assert!(!screen.exists(&format!("{card}-sections")), "its sections start folded");
  screen.click(&format!("{card}-head"));
  screen.until("its sections", |screen| screen.exists(&format!("{card}-sections")));
  let shown = screen.text(&card);
  for part in [
    format!("Daily report · {day}"),
    "What was done".to_owned(),
    "Nothing was merged, and the Orchestrator did nothing itself.".to_owned(),
    "How the work stands".to_owned(),
    "What waits for you".to_owned(),
    "Tokens and agent time".to_owned(),
    "Next in the queue".to_owned(),
    "Runs, waits and the queue as they stood at".to_owned(),
  ] {
    assert!(shown.contains(&part), "{part} in {shown}");
  }
  assert!(
    !shown.contains(&key),
    "a draft is not queued, so the queue does not name it: {shown}"
  );
  assert!(
    !shown.contains("adds a short summary"),
    "no summary while answer agents was off when it was kept: {shown}"
  );
}

#[test]
fn orchestrator_report_kept_to_start_a_turn_says_the_orchestrator_adds_a_summary() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (_, seq) = keep_report(&screen, true);
  screen.go(OrchestratorChat::Latest);
  let card = format!("orchestrator-report-wait-{seq}");
  screen.until("the report's line", |screen| screen.exists(&card));
  screen.click(&format!("{card}-head"));
  screen.until("its sections", |screen| screen.exists(&format!("{card}-sections")));
  let shown = screen.text(&card);
  assert!(
    shown.contains("Let the Orchestrator answer agents is on: the Orchestrator adds a summary")
      || shown.contains("adds a short summary and its recommendations below"),
    "{shown}"
  );
}
