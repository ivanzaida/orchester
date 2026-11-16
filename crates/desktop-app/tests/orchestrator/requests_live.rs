//! A live agent's tool request in the Orchestrator's conversation (2026-11-11, `ICzi3`), on the built runner with the
//! fake Claude ("Run `git push origin main`": it asks to run the push, which a hard rule of auto mode sends to the
//! person unreviewed): with *Let the Orchestrator drive* on, the request starts one Orchestrator turn by itself; the
//! stand-in decides nothing, so the request waits for the person with the task's own approval card under its line (d);
//! the person's *Approve once* there answers the agent and folds the line to *Allowed once by you* (e). Another task's
//! same request the Orchestrator denies with words for the agent (it may only deny what a hard rule holds): the agent
//! is told them, the line says *Denied by the Orchestrator* with what it told, and the action keeps the chat's reason.
//! Its approve_once of a push is refused (`person_only`: the runner lets only a person approve what auto-approval
//! leaves to one), the card stays for the person, and the action is kept as refused.
//! `cargo test -p orchester-app --test orchestrator requests_live`.

use orchester_core::{
  id::ResourceId,
  orchestrator::{ActionOutcome, EntryKind, WaitKind},
  run::{DecidedBy, DecidedToolApproval, Run, ToolApprovalDecision},
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{
  harness::{Orchestrator, Setup},
  waits::watching_everything,
};

/// The latest run of task `key`, once it has one.
fn latest_run(db: &ProjectDb, key: &str) -> Option<Run> {
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)?;
  db.runs_of_task(task.id()).expect("runs").pop()?.ok()
}

/// The waiting tool requests of task `key`'s latest run.
fn waiting(db: &ProjectDb, key: &str) -> Vec<String> {
  latest_run(db, key).map_or_else(Vec::new, |run| {
    db.pending_tool_approvals(run.id())
      .expect("requests")
      .into_iter()
      .map(|request| request.id)
      .collect()
  })
}

/// How request `id` of task `key`'s latest run was decided.
fn decided(db: &ProjectDb, key: &str, id: &str) -> DecidedToolApproval {
  let run = latest_run(db, key).expect("a run");
  db.decided_tool_approvals(run.id())
    .expect("decided")
    .into_iter()
    .find(|decided| decided.request.id == id)
    .expect("decided")
}

/// Whether the turn of the wait of request `id` in the latest conversation ended.
fn turn_ended(db: &ProjectDb, id: &str) -> bool {
  let Some(chat) = db.latest_chat().expect("read") else {
    return false;
  };
  let keys = db.chat_wait_keys(chat).expect("keys");
  db.chat_entries(chat).expect("entries").into_iter().any(|entry| {
    let ours = keys
      .iter()
      .any(|(seq, key)| *seq == entry.seq && key.ends_with(&format!(":{id}")));
    ours
      && matches!(&entry.kind, EntryKind::Wait { wait, turn: Some(turn) }
        if wait.kind == WaitKind::Approval && !turn.state.is_thinking())
  })
}

/// The refusal codes of the Orchestrator's `decide_approval` calls in the latest conversation, each kept without an
/// answer of its own.
fn refused_answers(db: &ProjectDb) -> Vec<String> {
  let Some(chat) = db.latest_chat().expect("read") else {
    return Vec::new();
  };
  db.chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) if action.tool == "decide_approval" => match action.outcome {
        ActionOutcome::Refused { code, .. } => {
          assert!(action.tool_answer.is_none(), "a refusal keeps no answer");
          Some(code)
        }
        _ => None,
      },
      _ => None,
    })
    .collect()
}

/// Starts a new task of `pipeline` whose agent asks to push, from a person's message; its key and its request, once
/// the request's own turn ended ([`asking`]).
fn pushing(screen: &mut Orchestrator, pipeline: ResourceId, title: &str) -> (String, String) {
  asking(screen, pipeline, (title, "Run `git push origin main`."))
}

/// Starts a new task `title` of `pipeline` whose `brief` has its agent ask for a tool, from a person's message; its
/// key and its request, once the request's own turn ended.
fn asking(screen: &mut Orchestrator, pipeline: ResourceId, (title, brief): (&str, &str)) -> (String, String) {
  let key = screen.draft_with(pipeline, title, (brief, &["It is done."]));
  screen.send(&format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#));
  screen.until("the agent's tool request", |screen| {
    !waiting(&screen.db(), &key).is_empty()
  });
  let request = waiting(&screen.db(), &key).remove(0);
  screen.until("the request's own turn", |screen| turn_ended(&screen.db(), &request));
  (key, request)
}

#[test]
fn requests_live_wait_for_the_person_with_the_card_and_take_both_answers() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.let_orchestrator_drive(true);
  // In Manual, the request comes to the person: Autopilot would answer it itself.
  screen.allow_autopilot(false);
  // A Builder that asks before commands, on a pipeline of its own.
  let builder = screen.builder();
  let pipeline = screen.pipeline(builder, ("asking", "Asking"));
  watching_everything(&mut screen);

  // The stand-in decides nothing, so it is left to the person with the card (d); their Approve once answers it (e).
  let (first_key, first) = pushing(&mut screen, pipeline, "Push the notes");
  let card = format!("cards-approval-{first}");
  screen.until("the task's own card under the request", |screen| screen.exists(&card));
  screen.press(&format!("{card}-approve"));
  screen.until("the person's answer on the line", |screen| !screen.exists(&card));
  let first = decided(&screen.db(), &first_key, &first);
  assert_eq!(
    (first.decision, first.decided_by),
    (Some(ToolApprovalDecision::Approved), Some(DecidedBy::Person))
  );

  // The runner lets only a person approve a push (auto-approval leaves history to a person): the Orchestrator's
  // approve_once is refused person_only, the card stays for the person, and its action is kept as refused, not as an
  // approval.
  let (second_key, second) = pushing(&mut screen, pipeline, "Push the log");
  let allow_push = format!(
    r#"Call decide_approval {{"project":"Kontur","approval":"{second}","decision":"approve_once","reason":"The brief asks to push."}}"#
  );
  screen.send(&allow_push);
  screen.until("the approval refused", |screen| {
    refused_answers(&screen.db()).iter().any(|code| code == "person_only")
  });
  assert_eq!(
    waiting(&screen.db(), &second_key),
    vec![second.clone()],
    "it still waits"
  );
  let second_card = format!("cards-approval-{second}");
  screen.until("the card stays for the person", |screen| screen.exists(&second_card));

  // The Orchestrator denies it, with words for the agent and its reason.
  let deny = format!(
    r#"Call decide_approval {{"project":"Kontur","approval":"{second}","decision":"deny","message":"Do not push from a run.","reason":"The brief leaves pushing to review."}}"#
  );
  screen.send(&deny);
  screen.until("the agent told", |screen| waiting(&screen.db(), &second_key).is_empty());
  let denied = decided(&screen.db(), &second_key, &second);
  assert_eq!(denied.decision, Some(ToolApprovalDecision::Denied));
  assert_eq!(
    denied.note.as_ref().map(|note| note.as_str()),
    Some("Do not push from a run.")
  );
  let db = screen.db();
  let chat = db.latest_chat().expect("read").expect("the conversation");
  let answer = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| match entry.kind {
      EntryKind::Action(action) if action.tool == "decide_approval" && action.outcome == ActionOutcome::Done => {
        action.tool_answer
      }
      _ => None,
    })
    .expect("the Orchestrator's answer kept");
  assert_eq!(answer.request, second);
  assert!(!answer.allowed);
  assert_eq!(answer.reason, "The brief leaves pushing to review.");
  assert_eq!(answer.told.as_deref(), Some("Do not push from a run."));

  // A hold is no refusal (the person's decision of 2026-11-11): the Orchestrator may allow a held request once; here a
  // folder made in a step that only reads, which auto mode's hard rules send to the person.
  let (third_key, third) = asking(&mut screen, pipeline, ("Make the output folder", "Make a folder."));
  let allow = format!(
    r#"Call decide_approval {{"project":"Kontur","approval":"{third}","decision":"approve_once","reason":"The brief asks for the output folder."}}"#
  );
  screen.send(&allow);
  screen.until("the agent answered", |screen| {
    waiting(&screen.db(), &third_key).is_empty()
  });
  let allowed = decided(&screen.db(), &third_key, &third);
  assert_eq!(
    (allowed.decision, allowed.decided_by),
    (Some(ToolApprovalDecision::Approved), Some(DecidedBy::Automation))
  );
}
