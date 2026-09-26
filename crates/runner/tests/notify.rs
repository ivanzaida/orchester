//! Coalescing the notifications waiting to be sent to the desktop.

use std::num::NonZeroU32;

use orchester_core::id::{RunId, TaskId};
use orchester_ipc::{Event, Reply, RequestId, WorkSummary};
use orchester_runner::notify::coalesce;

#[test]
fn notify_coalesces_changes_per_run_and_keeps_replies() {
  let (first, second, task) = (RunId::generate(), RunId::generate(), TaskId::generate());
  let changed = |run| Event::RunChanged { run, task };
  let activity = |run, sequence| Event::AgentActivity { run, sequence };
  let reply = Event::Reply {
    id: RequestId::new(3),
    reply: Reply::Accepted { run: first },
  };
  let waiting = vec![
    changed(first),
    activity(first, 4),
    changed(second),
    changed(first),
    reply.clone(),
    activity(first, 9),
    activity(second, 2),
    changed(second),
  ];
  assert_eq!(
    coalesce(waiting),
    [
      changed(first),
      activity(first, 9),
      changed(second),
      reply,
      activity(second, 2)
    ]
  );
}

#[test]
fn notify_keeps_only_the_latest_work_summary() {
  let run = RunId::generate();
  let summary = |slots| {
    Event::WorkSummary(WorkSummary {
      agent_slots: NonZeroU32::new(slots).expect("above zero"),
      runs: Vec::new(),
    })
  };
  let changed = Event::RunChanged {
    run,
    task: TaskId::generate(),
  };
  assert_eq!(
    coalesce(vec![summary(1), changed.clone(), summary(2), summary(3)]),
    [changed, summary(3)]
  );
}
