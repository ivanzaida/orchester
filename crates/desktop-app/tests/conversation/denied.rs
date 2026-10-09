//! A command a person denied: the CLI ends the call under other words than it started with (a declined
//! escalation), and its problem row still says the person denied it, not the run's permissions.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus},
  permission::category::CommandCategory,
  run::{DecidedBy, ToolDecision},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{all_text, key_of, long::at_work, open, seeded, with_text};

#[test]
fn timeline_a_command_a_person_denied_says_so() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let invocation = at_work(&db, &key);
  let at = |offset: i64| Timestamp::from_unix_millis(Timestamp::now().unix_millis() + offset);
  let item = "escalation-1".to_owned();
  let events = vec![
    (
      at(0),
      AgentEvent::CommandStarted {
        item: item.clone(),
        command: "cargo publish --dry-run".into(),
      },
    ),
    (
      at(1),
      AgentEvent::ToolDecided {
        item: item.clone(),
        server: "command".into(),
        action: "cargo publish --dry-run".into(),
        decision: ToolDecision {
          decided_by: DecidedBy::PersonDeny,
          category: Some(CommandCategory::Network),
          secrets: false,
          told: None,
          waited_ms: None,
          review: None,
        },
      },
    ),
    (
      at(2),
      AgentEvent::CommandFinished {
        item,
        command: "escalated".into(),
        output: String::new(),
        exit_code: None,
        status: CommandStatus::Declined,
      },
    ),
  ];
  db.append_activity(&invocation, &events)
    .expect("the denial is recorded");
  open(&mut window, &seeded, &key, "conversation-chat");
  window.until("the denied command", |window| {
    all_text(window).contains("cargo publish --dry-run")
  });
  let row = with_text(&mut window, "conversation-problem-", "cargo publish --dry-run");
  let text = window.texts(&row).expect("the problem row").join(" ");
  assert!(text.contains("denied"), "{text}");
  assert!(
    text.contains("You denied it."),
    "the person's answer, not the run's permissions: {text}"
  );
}
