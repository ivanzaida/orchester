//! How a run that stopped went on, when the Orchestrator chat decided it (ORC-45), on the seeded Kontur's run that
//! waits for a decision after its retries: the return its `continue_run` proposal started once the person applied it
//! is the Orchestrator's in the task's conversation, with its note in the Orchestrator's row, how it went out and the
//! way back to the chat, never the person's bubble; `get_conversation` names the Orchestrator as its author. Only a
//! decision carried out (by the chat itself, or applied by the person) on the attempt the run stopped in names the
//! chat: one dismissed, refused or still waiting started nothing.

use orchester_core::{
  id::{ChatId, RunId},
  orchestrator::{
    ActionOutcome, ChatAction, EntryKind, FromOrchestrator, SentWay,
    change::{Change, ContinueChoice, ProposedChange},
  },
  resource::ModelId,
  run::{ReviewNote, StuckDecision},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::{KeptAction, ProjectDb};
use orchester_workspace::{
  orchestrator::tools::ChatScope,
  tasks::{
    conversation::orchestrator::continued,
    timeline::{
      Item, ItemKind,
      mark::{Mark, MarkKind},
    },
  },
};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
  orchestrator_sent::{entries, open_conversation},
};

/// The seeded task whose run waits for a decision after Review used up its retries.
const STUCK: &str = "Cache GitHub API responses";
const NOTE: &str = "Keep the ETag per URL.";

/// The seq of the last action of `scope`'s conversation.
fn last_action(db: &ProjectDb, scope: &ChatScope) -> i64 {
  db.chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .map(|entry| entry.seq)
    .next_back()
    .expect("an action")
}

#[test]
fn continued_return_the_person_applied_is_the_orchestrators() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, mut run) = task_run(&db, STUCK);
  let scope = chat(&served, &mut db);
  let target = run
    .return_targets()
    .into_iter()
    .next()
    .expect("an earlier step to return to");
  let back = json!({
    "project": "Kontur", "run": run.number().get().to_string(),
    "choice": { "kind": "return_to", "step": target.as_str(), "note": NOTE },
  });
  chat_call(&mut served, &scope, "continue_run", back).expect("proposed");
  let seq = last_action(&db, &scope);

  // The person applies it: the decision claims the proposal, the runner returns the work as the person's decision and
  // the new attempt's agent starts, and the proposal is settled as applied. (This app has no runner: done as it does.)
  let now = Timestamp::now();
  db.claim_chat_action((scope.chat, seq), now)
    .expect("claimed")
    .expect("it waited");
  let note = ReviewNote::new(NOTE).expect("a note");
  let decision = StuckDecision::ReturnTo { step: target, note };
  let events = run.decide_stuck(&decision, now).expect("returned");
  db.append_events(&run, &events).expect("recorded");
  let attempt = run.current_attempt().expect("the new attempt");
  let (number, first) = (attempt.number().get(), attempt.executions()[0].id());
  let model = ModelId::new("gpt-5.5").expect("a model");
  db.start_invocation(run.id(), first, &model, now)
    .expect("its agent starts");
  db.finish_chat_action((scope.chat, seq), (ActionOutcome::Applied, None), now)
    .expect("settled")
    .expect("it was claimed");
  assert_eq!(
    actions(&db, &scope).last().map(|action| action.outcome.clone()),
    Some(ActionOutcome::Applied)
  );

  let key = task.key().to_string();
  let read = entries(&mut served, &key);
  let continued = read
    .iter()
    .find(|entry| entry["kind"] == "run_continued" && entry["attempt"] == number)
    .unwrap_or_else(|| panic!("the return: {read:?}"));
  assert_eq!(
    continued["author"],
    json!({ "kind": "orchestrator", "sent": "applied_by_person", "chat": scope.chat.to_string() })
  );
  assert_eq!(
    (&continued["continued"], &continued["note"]),
    (&json!("returned_with_note"), &json!(NOTE))
  );

  open_conversation(&mut served, seeded.project, &key);
  let id = format!("conversation-mark-note-{number}");
  served.until("the Orchestrator's row under the return", |served| {
    served.tree.get_element_by_id(&format!("{id}-row")).is_some()
  });
  assert!(
    served.tree.get_element_by_id(&format!("{id}-sent")).is_some()
      && served.tree.get_element_by_id(&format!("{id}-open-chat")).is_some(),
    "how it went out, and the way back to the chat"
  );
}

/// The chat's decision on run `run` stopped in attempt `stopped`, kept as `seq` and gone as `outcome`.
fn kept(run: RunId, (seq, stopped): (i64, u32), outcome: ActionOutcome) -> KeptAction {
  let mut action = ChatAction::new("continue_run", Some("KON-1".to_owned()), outcome);
  action.change = Some(Box::new(ProposedChange {
    subject: "Cache GitHub API responses".to_owned(),
    change: Change::Continue {
      run,
      number: 1,
      attempt: stopped,
      step: None,
      why: None,
      choice: ContinueChoice::AllowRetry,
    },
  }));
  KeptAction {
    chat: ChatId::generate(),
    seq,
    at: Timestamp::from_unix_millis(seq),
    action,
  }
}

/// A restart that started attempt `attempt`, decided by the person.
fn restart(attempt: u32) -> Item {
  Item {
    key: format!("mark-{attempt}").into(),
    execution: None,
    at: Timestamp::from_unix_millis(i64::from(attempt)),
    kind: ItemKind::Mark(Mark {
      attempt,
      at: None,
      by_person: true,
      by: Some(orchester_core::run::Answerer::Person),
      kind: MarkKind::Restarted { step: "Review".into() },
      orchestrator: None,
    }),
  }
}

fn named(items: &[Item]) -> Option<FromOrchestrator> {
  match &items[0].kind {
    ItemKind::Mark(mark) => mark.orchestrator,
    _ => None,
  }
}

#[test]
fn continued_only_what_the_chat_carried_out_on_the_attempt_it_stopped_in() {
  let run = RunId::generate();
  let refused = ActionOutcome::Refused {
    code: "changed_since_proposed".to_owned(),
    reason: "The run moved on.".to_owned(),
  };
  for (outcome, way) in [
    (ActionOutcome::Applied, Some(SentWay::AppliedByPerson)),
    (ActionOutcome::Done, Some(SentWay::Direct)),
    (ActionOutcome::Dismissed, None),
    (ActionOutcome::Waits, None),
    (refused, None),
  ] {
    let actions = [kept(run, (1, 2), outcome.clone())];
    let mut items = [restart(3)];
    continued(&mut items, &actions);
    assert_eq!(
      named(&items),
      way.map(|way| FromOrchestrator::new(actions[0].chat, way)),
      "{outcome:?}"
    );
  }
  // A decision on another attempt started another one: this restart stays the person's.
  let mut items = [restart(4)];
  continued(&mut items, &[kept(run, (1, 2), ActionOutcome::Applied)]);
  assert_eq!(named(&items), None);
}
