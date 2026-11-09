//! The Orchestrator chat follows events of its project's runs besides waits and ends (ORC-755): a run that made no
//! progress for a while, by a pure rule that fires once per stall; a merge that hit conflicts; and the task a run
//! waits on merged or closed without merging. Each is a wait entry with its own turn prompt, which carries the order of
//! the conversation that leads the task.

use orchester_core::{
  id::{RunId, TaskId},
  orchestrator::{
    AgentWait, ChatEntry, EntryKind, TaskEvent, TaskEventKind, WaitKind, WatchSettings,
    instructions::{event_prompt, wait_prompt},
    task_event::{no_progress, raised_stall},
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
      unled.contains("Let the Orchestrator drive is on") && !unled.contains("leads the task"),
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
