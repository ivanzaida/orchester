//! Coalescing the notifications waiting to be sent to the desktop.

use orchester_core::id::{RunId, TaskId};
use orchester_ipc::{Event, Reply, RequestId};
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
