//! @-mentions of tasks and runs (ORC-312): how a draft holds them, what the @ menu offers and filters, and the
//! marked block of facts the agent receives with a message, which the conversation leaves out of what it shows.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  project::TaskKey,
  run::{RunNumber, TaskPhase},
  store::TaskStore,
  task::{TaskDetails, organization::WorkflowStatus},
};
use orchester_workspace::mentions::{Mention, catalog::Catalog, ending_at, facts, find, typed_at};

fn key(text: &str) -> TaskKey {
  text.parse().expect("a task key")
}

fn run(number: u32) -> RunNumber {
  RunNumber::new(NonZeroU32::new(number).expect("above zero"))
}

#[test]
fn mentions_are_found_where_a_word_starts() {
  let text = "Compare @ORC-12 with @ORC-12#9, not mail@ORC-3 or @ORC-4x or @orc-5; (@ORC-6)";
  let found: Vec<(Mention, &str)> = find(text)
    .into_iter()
    .map(|found| (found.mention, &text[found.range]))
    .collect();
  assert_eq!(
    found,
    vec![
      (Mention::Task(key("ORC-12")), "@ORC-12"),
      (Mention::Run(key("ORC-12"), run(9)), "@ORC-12#9"),
      (Mention::Task(key("ORC-6")), "@ORC-6"),
    ]
  );
  assert_eq!(Mention::Run(key("ORC-12"), run(9)).written(), "@ORC-12#9");
}

#[test]
fn mentions_backspace_takes_the_mention_that_ends_at_the_caret() {
  let text = "See @ORC-12#9 now";
  let end = "See @ORC-12#9".len();
  let found = ending_at(text, end).expect("the run's mention ends there");
  assert_eq!(found.range, 4..end);
  assert_eq!(ending_at(text, end + 1), None, "after the space there is none");
  assert_eq!(ending_at(text, end - 1), None, "inside the mention there is none");
}

#[test]
fn mentions_menu_types_what_follows_an_at_up_to_the_caret() {
  let text = "Compare the retry with @send";
  let typed = typed_at(text, text.len()).expect("the menu is open");
  assert_eq!(typed.query, "send");
  assert_eq!(&text[typed.range()], "@send");
  assert_eq!(typed_at("Compare @", 9).map(|typed| typed.query), Some(""));
  assert_eq!(typed_at("mail@exa", 8), None, "an @ inside a word");
  assert_eq!(typed_at("@send it", 8), None, "a space after the words typed");
  assert_eq!(typed_at("@ORC-12", 7), None, "a whole mention is a chip already");
}

/// Three tasks: ORC-1 *Add retry to webhook sender* with run #1 waiting, ORC-2 *Resend failed invitations* with a
/// brief and run #2 running, ORC-3 *Rate-limit the public API* without a run.
fn seeded() -> support::Fixture {
  let mut fixture = support::Fixture::new();
  let first = fixture.queued_task("Add retry to webhook sender", support::at(10));
  let second = fixture.task("Resend failed invitations", support::at(20));
  fixture.task("Rate-limit the public API", support::at(30));
  let mut task = fixture.db.task(second).expect("read").expect("the task");
  let details = TaskDetails {
    brief: "Resend invitations that bounced.\n\nKeep the original sender.\nLog each resend.\nSkip spam.".to_owned(),
    ..task.details().clone()
  };
  task.edit(details, false).expect("a draft takes any brief");
  fixture.db.save_task(&task).expect("save").saved().expect("no conflict");
  fixture.pending_run(first, support::at(40));
  fixture.started_run(second, support::at(50));
  fixture
}

#[test]
fn mentions_menu_offers_recent_tasks_and_runs_and_filters_them() {
  let fixture = seeded();
  let catalog = Catalog::read(&fixture.db).expect("read");
  let all = catalog.offer("", None);
  let tasks: Vec<String> = all.tasks.shown.iter().map(|task| task.key.to_string()).collect();
  assert_eq!(
    tasks,
    ["ORC-2", "ORC-3", "ORC-1"],
    "most recently changed first: a started run moved ORC-2 to In progress"
  );
  let runs: Vec<u32> = all.runs.shown.iter().map(|run| run.number.get()).collect();
  assert_eq!(runs, [2, 1], "latest first");
  assert_eq!(all.runs.shown[0].phase, TaskPhase::Running);
  assert_eq!(all.runs.shown[1].phase, TaskPhase::Queued);
  assert_eq!(all.tasks.shown[0].status, WorkflowStatus::InProgress);
  assert_eq!(all.tasks.shown[1].status, WorkflowStatus::Backlog);

  let resend = catalog.offer("RESEND", None);
  assert_eq!(resend.tasks.total, 1, "by title, ignoring case");
  assert_eq!(resend.runs.shown[0].key, key("ORC-2"), "a run by its task's title");
  let by_key = catalog.offer("orc-1", None);
  assert_eq!(by_key.tasks.shown[0].key, key("ORC-1"));
  let by_number = catalog.offer("#2", None);
  assert_eq!(by_number.runs.total, 1);
  assert_eq!(by_number.runs.shown[0].number, run(2));
  let none = catalog.offer("payrol", None);
  assert!(none.is_empty());
  assert_eq!(none.matches(), 0);

  let own = catalog.offer("", Some((&key("ORC-1"), Some(run(1)))));
  assert_eq!(own.tasks.shown[0].key, key("ORC-1"), "a run's own task leads");
  assert_eq!(own.runs.shown[0].number, run(1), "and its run");
  assert_eq!(
    own.mentions()[..2],
    [Mention::Task(key("ORC-1")), Mention::Task(key("ORC-2"))]
  );
}

#[test]
fn mentions_facts_follow_the_message_in_a_marked_block() {
  let fixture = seeded();
  let catalog = Catalog::read(&fixture.db).expect("read");
  let text = "Use the backoff from @ORC-2 as run @ORC-2#2 did, not @ORC-9 or @ORC-1#7. Again @ORC-2.";
  let sent = facts::with_facts(&catalog, text);
  let (said, block) = sent.split_once(facts::OPEN).expect("the block");
  assert_eq!(said.trim_end(), text, "the person's words come first, as written");
  assert!(sent.ends_with(facts::CLOSE));
  assert!(block.contains("not the person's words"), "{block}");
  assert!(
    block.contains("- @ORC-2: task ORC-2 \"Resend failed invitations\", workflow status In progress."),
    "{block}"
  );
  assert!(block.contains("  > Resend invitations that bounced."), "{block}");
  assert!(block.contains("  > Log each resend."), "{block}");
  assert!(!block.contains("Skip spam"), "only the brief's first lines: {block}");
  assert!(block.contains("(the brief goes on)"), "{block}");
  assert!(
    block.contains(
      "- @ORC-2#2: run #2 of task ORC-2 \"Resend failed invitations\", state running, branch orchester/orc-run-2."
    ),
    "{block}"
  );
  assert!(block.contains("- @ORC-9: no task ORC-9 in this project."), "{block}");
  assert!(
    block.contains("- @ORC-1#7: no run #7 of ORC-1 in this project."),
    "{block}"
  );
  assert_eq!(block.matches("- @ORC-2:").count(), 1, "each mention once");

  assert_eq!(facts::said(&sent), text, "a conversation shows the person's words");
  assert_eq!(facts::said(text), text);
  assert_eq!(facts::with_facts(&catalog, "No mention."), "No mention.");
  let stored = facts::with_facts_in(fixture.repo.path(), "See @ORC-3").expect("read");
  assert!(
    stored.contains("- @ORC-3: task ORC-3 \"Rate-limit the public API\""),
    "{stored}"
  );
  assert!(stored.contains("Brief: none."), "{stored}");
}
