//! The Orchestrator chat follows events of its project's runs besides waits and ends (ORC-755): a run that made no
//! progress for a while, by a pure rule that fires once per stall; a merge that hit conflicts; and the task a run
//! waits on merged or closed without merging. Each is a wait entry with its own turn prompt, which carries the order of
//! the conversation that leads the task; an event the Orchestrator's own doing raised starts no turn.

use orchester_core::{
  id::{RunId, TaskId},
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, TaskEvent, TaskEventKind, WaitKind, WatchSettings,
    change::{Change, ProposedChange},
    instructions::{event_prompt, wait_prompt},
    task_event::{no_progress, raised_stall},
    wait::raised_by_own,
  },
  time::Timestamp,
};

const MINUTE: i64 = 60_000;

fn at(minutes: i64) -> Timestamp {
  Timestamp::from_unix_millis(1_000_000 + minutes * MINUTE)
}

fn happened(run: RunId, kind: TaskEventKind) -> AgentWait {
  AgentWait {
    kind: WaitKind::Event(TaskEvent { kind, at: at(30) }),
    task: "KON-41".to_owned(),
    title: "Webhooks v2".to_owned(),
    run,
    number: 17,
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: "/project/p/tasks/KON-41/summary".to_owned(),
    own: false,
    automatic: false,
    continuing: None,
    merging: None,
  }
}

fn entry(seq: i64, kind: EntryKind) -> ChatEntry {
  ChatEntry {
    seq,
    at: Timestamp::from_unix_millis(seq * 10),
    kind,
  }
}

fn kept(seq: i64, wait: &AgentWait) -> ChatEntry {
  entry(
    seq,
    EntryKind::Wait {
      wait: wait.clone(),
      turn: None,
    },
  )
}

/// The chat's `change`, by tool `tool`, gone as `outcome`.
fn acted(tool: &str, change: Change, outcome: ActionOutcome) -> ChatEntry {
  entry(
    1,
    EntryKind::Action(ChatAction {
      change: Some(Box::new(ProposedChange {
        subject: "Webhooks v2".to_owned(),
        change,
      })),
      ..ChatAction::new(tool, Some("KON-41".to_owned()), outcome)
    }),
  )
}

fn merge(run: RunId) -> Change {
  Change::Merge {
    run,
    number: 1,
    checkpoint: None,
    into: None,
  }
}

fn stalled() -> TaskEventKind {
  TaskEventKind::NoProgress {
    since: at(0),
    minutes: 20,
  }
}

#[test]
fn task_event_no_progress_fires_after_its_minutes_once_per_stall() {
  let after = WatchSettings::default().no_progress_minutes();
  assert_eq!(after, Some(20), "20 minutes when the person set nothing");
  assert_eq!(no_progress(at(0), at(19), after, None), None, "not yet");
  assert_eq!(
    no_progress(at(0), at(20), after, None),
    Some(stalled()),
    "20 minutes without progress"
  );
  assert_eq!(
    no_progress(at(0), at(45), after, Some(at(0))),
    None,
    "one event per stall: it was raised for this one"
  );
  assert_eq!(
    no_progress(at(30), at(50), after, Some(at(0))),
    Some(TaskEventKind::NoProgress {
      since: at(30),
      minutes: 20
    }),
    "progress again re-armed it"
  );
  assert_eq!(no_progress(at(0), at(500), None, None), None, "off never fires");
  assert!(
    no_progress(at(0), at(5), Some(5), None).is_some(),
    "after the minutes set"
  );
}

#[test]
fn task_event_no_progress_raised_for_a_run_is_read_from_its_kept_events() {
  let (run, other) = (RunId::generate(), RunId::generate());
  let later = TaskEventKind::NoProgress {
    since: at(30),
    minutes: 20,
  };
  let entries = [
    kept(1, &happened(run, stalled())),
    kept(2, &happened(run, later)),
    kept(3, &happened(other, TaskEventKind::MergeConflict)),
  ];
  assert_eq!(raised_stall(&entries, run), Some(at(30)), "its newest stall");
  assert_eq!(raised_stall(&entries, other), None, "a conflict is no stall");
  assert_eq!(raised_stall(&[], run), None);
}

#[test]
fn task_event_raised_by_the_orchestrators_own_doing_starts_no_turn() {
  let (run, waited) = (RunId::generate(), RunId::generate());
  let task = TaskId::generate();
  let message = Change::Message {
    run,
    number: 17,
    text: "Go on.".to_owned(),
    now: false,
    then: None,
    sent_as: None,
  };
  let cancel = |run| Change::Cancel { run, number: 1 };
  let refused = |code: &str| ActionOutcome::Refused {
    code: code.to_owned(),
    reason: "Merging stopped at conflicts in src/hooks.rs.".to_owned(),
  };
  let cases = [
    (
      stalled(),
      acted("send_message", message.clone(), ActionOutcome::Done),
      true,
    ),
    (stalled(), acted("send_message", message, ActionOutcome::Waits), false),
    (
      TaskEventKind::MergeConflict,
      acted("merge_run", merge(run), refused("merge_failed")),
      true,
    ),
    (
      TaskEventKind::MergeConflict,
      acted("merge_run", merge(run), refused("needs_decision")),
      false,
    ),
    (
      TaskEventKind::MergeConflict,
      acted("merge_run", merge(run), ActionOutcome::Dismissed),
      false,
    ),
    (
      TaskEventKind::MergeConflict,
      acted("merge_run", merge(run), ActionOutcome::Done),
      false,
    ),
    (
      TaskEventKind::MergeConflict,
      acted("merge_run", merge(waited), refused("merge_failed")),
      false,
    ),
    (
      TaskEventKind::WaitedMerged { task, run: waited },
      acted("merge_run", merge(waited), ActionOutcome::Done),
      true,
    ),
    (
      TaskEventKind::WaitedMerged { task, run: waited },
      acted("merge_run", merge(run), ActionOutcome::Done),
      false,
    ),
    (
      TaskEventKind::WaitedClosed {
        task,
        run: Some(waited),
      },
      acted("cancel_run", cancel(waited), ActionOutcome::Done),
      true,
    ),
    (
      TaskEventKind::WaitedClosed { task, run: None },
      acted("cancel_run", cancel(waited), ActionOutcome::Done),
      false,
    ),
  ];
  for (kind, action, own) in cases {
    assert_eq!(
      raised_by_own(std::slice::from_ref(&action), &happened(run, kind)),
      own,
      "{kind:?} after {:?}",
      action.kind
    );
  }
}

#[test]
fn task_event_after_a_newer_wait_of_the_run_or_its_start_is_not_own() {
  let run = RunId::generate();
  let entries = [
    acted(
      "merge_run",
      merge(run),
      ActionOutcome::Refused {
        code: "merge_failed".to_owned(),
        reason: "Merging stopped at conflicts.".to_owned(),
      },
    ),
    kept(2, &happened(run, TaskEventKind::MergeConflict)),
  ];
  assert!(
    !raised_by_own(&entries, &happened(run, TaskEventKind::MergeConflict)),
    "the conflict of a later merge follows the event kept since, not the chat's merge"
  );
  let started = [entry(
    1,
    EntryKind::Action(ChatAction {
      started: Some(run),
      ..ChatAction::new("run_task", Some("KON-41".to_owned()), ActionOutcome::Done)
    }),
  )];
  assert!(
    !raised_by_own(&started, &happened(run, stalled())),
    "a run the chat started that stalls is news to the conversation that leads it"
  );
}

#[test]
fn task_event_prompt_says_what_happened_and_the_order_of_the_leading_conversation() {
  let run = RunId::generate();
  let task = TaskId::generate();
  let order = "Keep the module moving; ask me before canceling anything.";
  let cases = [
    (stalled(), "made no progress for 20 minutes", "send_message"),
    (TaskEventKind::MergeConflict, "hit conflicts", "resolve_conflicts"),
    (
      TaskEventKind::WaitedMerged { task, run },
      "which was merged",
      "can go on",
    ),
    (
      TaskEventKind::WaitedClosed { task, run: None },
      "closed without merging",
      "run that task again",
    ),
  ];
  for (kind, says, todo) in cases {
    let wait = happened(run, kind);
    let event = wait.kind.event().expect("an event");
    let led = event_prompt(&wait, event, Some(order));
    for part in [
      "[Orchester]",
      says,
      "run #17 of KON-41 (Webhooks v2)",
      "/project/p/tasks/KON-41/summary",
      "this conversation leads the task",
      order,
      todo,
    ] {
      assert!(led.contains(part), "{kind:?}: {part} in {led}");
    }
    let unled = wait_prompt(&wait);
    assert_eq!(
      unled,
      event_prompt(&wait, event, None),
      "a wait's prompt of an event has no order"
    );
    assert!(unled.contains(says) && unled.contains(todo), "{kind:?}: {unled}");
    assert!(
      unled.contains("Let the Orchestrator answer agents is on") && !unled.contains("leads the task"),
      "{kind:?}: {unled}"
    );
  }
  let waited = event_prompt(
    &happened(run, TaskEventKind::WaitedMerged { task, run }),
    TaskEvent {
      kind: TaskEventKind::WaitedMerged { task, run },
      at: at(30),
    },
    None,
  );
  assert!(waited.contains(&task.to_string()), "names the waited task: {waited}");
}

#[test]
fn task_event_is_kept_as_a_wait_that_reads_back_the_same() {
  let wait = happened(
    RunId::generate(),
    TaskEventKind::WaitedClosed {
      task: TaskId::generate(),
      run: None,
    },
  );
  let json = serde_json::to_string(&wait).expect("serializes");
  let back: AgentWait = serde_json::from_str(&json).expect("reads back");
  assert_eq!(back, wait);
  assert_eq!(back.kind.run_end(), None, "an event is no run's end");
}
