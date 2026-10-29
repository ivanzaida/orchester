//! A card of a run the Orchestrator's agent proposes to send back to resolve its conflicts (`continue_run`'s
//! `resolve_conflicts`, `sXEjJ`): its Note reads as one paragraph, the conflict's own words and then the chat's
//! note, a few lines high beside its key.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction, EntryKind,
    change::{Change, ContinueChoice, ProposedChange, StepName},
  },
  store::{RunStore, TaskStore},
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card},
};

/// What the chat adds for the agent, long enough to wrap over a few lines.
const NOTE: &str = "Keep master's split of the settings into delivery.rs and retry.rs; move retry_budget into \
                    retry.rs, keep the defaults master chose, and run the webhook tests once the merge is committed \
                    so that both sides' retries still pass.";

/// The resolution of run `number` (`run`) of the task, as the chat keeps it while it waits for the person.
fn resolution(run: orchester_core::id::RunId, number: u32) -> ChatAction {
  let choice = ContinueChoice::ResolveConflicts {
    step: "implement".to_owned(),
    to: Some(StepName {
      place: 1,
      name: "Implement".to_owned(),
    }),
    base: "master".to_owned(),
    files: vec![
      "src/webhooks/sender.rs".to_owned(),
      "src/webhooks/settings.rs".to_owned(),
    ],
    note: Some(NOTE.to_owned()),
  };
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Continue {
        run,
        number,
        attempt: 1,
        step: None,
        why: None,
        choice,
      },
    })),
    ..ChatAction::new("continue_run", None, ActionOutcome::Waits)
  }
}

#[test]
fn orchestrator_resolve_card_note_reads_as_one_paragraph() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_with(delivery, "Add retry", ("Add a retry.", &["It retries."]));
  screen.allow_autopilot(true);
  screen.go(OrchestratorChat::New);
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the started run", |screen| outcomes(screen).len() == 1);
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("a readable run");
  let chat = db.chats().expect("chats").remove(0).chat.id();
  screen
    .db()
    .add_chat_action(chat, &resolution(run.id(), run.number().get()), Timestamp::now())
    .expect("kept");
  let root = screen.root.clone();
  screen
    .window
    .launch
    .services()
    .expect("the session started")
    .runners
    .chat_changed(&root);
  let parts = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .rev()
    .take_while(|entry| !matches!(entry.kind, EntryKind::Person { .. }))
    .count();
  let turn = proposal_card(&screen);
  let prefix = turn.trim_end_matches("-0").to_owned();
  // The card is the answer's part that asks to resolve; a run's end may land among the parts before it.
  let mut card = String::new();
  screen.until("the card's facts", |screen| {
    card = (0..=parts)
      .map(|index| format!("{prefix}-{index}"))
      .find(|id| screen.exists(&format!("{id}-details")))
      .unwrap_or_default();
    !card.is_empty()
  });
  // Run, Choice, Conflicting files, then the Note behind *Show details*.
  let note = format!("{card}-fact-3");
  assert!(
    !screen.exists(&note),
    "three facts show, the rest wait behind Show details"
  );
  screen.click(&format!("{card}-details"));
  screen.until("the resolution's note", |screen| screen.exists(&note));

  let shown = screen.text(&note);
  assert!(
    shown.contains("resolve the conflicts keeping both sides' intent") && shown.contains("so that both sides' retries"),
    "the conflict's words and the chat's note: {shown}"
  );
  // Beside the Choice row, which reads on one line.
  let (row, line) = (screen.bounds(&note), screen.bounds(&format!("{card}-fact-1")));
  assert!(
    row.height < line.height * 6.0,
    "a few lines, not a column of single letters: {} high beside a {} high line",
    row.height,
    line.height
  );
}
