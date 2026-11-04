//! The context meter of the step at work (ORC-308): on its divider and in the composer's bar, unknown until its agent's
//! CLI reports a reading, then plenty, near full and full as each turn's reading arrives, the popover following it. A
//! finished step's divider, an earlier execution's and a tool step's carry none.

use orchester_core::{
  connection::context_use::ContextUse,
  run::InvocationKind,
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::detail::TaskDetail;

use super::{Seeded, Window, click, ids, key_of, open, seeded, wheel};

const TASK: &str = "Add retry to webhook sender";
const COMPOSER: &str = "conversation-context-composer";

/// Keeps `reading` for the open agent invocation of `TASK`'s run, and counts the transcript change its runner reports.
fn turn(window: &Window, seeded: &Seeded, reading: &ContextUse) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database");
  let key = key_of(seeded, TASK);
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| *task.key() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next_back()
    .expect("its latest run");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .rev()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  db.save_invocation_context(invocation.id, reading, Timestamp::now())
    .expect("the reading is kept");
  let services = window.launch.services().expect("the session started");
  let root = window
    .projects()
    .state(seeded.project)
    .and_then(|state| state.open().map(|open| open.repo_root.clone()))
    .expect("the project is open");
  services.runners.count_change_for_test(&root, false);
}

/// The ids of the step meters the chat shows, without their parts.
fn step_meters(window: &mut Window) -> Vec<String> {
  ids(window, "conversation-context-step-")
    .into_iter()
    .filter(|id| id.len() == "conversation-context-step-00".len())
    .collect()
}

/// The meters' expected id: the step of `TASK`'s run whose session is at work, by the workspace's rule.
fn expected_meter(seeded: &Seeded) -> String {
  let db = ProjectDb::open(&seeded.repo).expect("the project database");
  let key = key_of(seeded, TASK);
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| *task.key() == key)
    .expect("the task");
  let detail = TaskDetail::load_in(&seeded.repo, task.id()).expect("the task reads");
  let run = detail.run.expect("its run");
  let at_work: Vec<usize> = run.sessions_at_work().map(|step| step.number).collect();
  assert_eq!(at_work.len(), 1, "one agent at work in the scenario");
  assert!(
    run
      .steps
      .iter()
      .any(|step| step.status.is_some() && !run.sessions_at_work().any(|at| at.key == step.key)),
    "the run has steps that are not at work"
  );
  format!("conversation-context-step-{:02}", at_work[0])
}

#[test]
fn conversation_context_meter_of_the_step_at_work_follows_each_turn() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, TASK);
  open(&mut window, &seeded, &key, COMPOSER);
  let step = expected_meter(&seeded);
  // The step's divider sits above its messages: the chat, at its end, may have to scroll up to build it.
  window.until("the chat's rows", |window| {
    !ids(window, "conversation-row:").is_empty() && window.tree.get_element_by_id("conversation-chat").is_some()
  });
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
    if !step_meters(&mut window).is_empty() {
      break;
    }
    wheel(&mut window, 120.0);
  }
  window.until("the step's meter", |window| !step_meters(window).is_empty());
  assert_eq!(
    step_meters(&mut window),
    std::slice::from_ref(&step),
    "only the divider of the step at work, not a finished step's or an earlier execution's"
  );
  for meter in [COMPOSER, step.as_str()] {
    assert!(
      window.tree.get_element_by_id(&format!("{meter}-ring")).is_some(),
      "{meter}: a ring"
    );
    assert!(
      window.tree.get_element_by_id(&format!("{meter}-share")).is_none(),
      "{meter}: unknown until the CLI reports a reading"
    );
  }
  let turns = [
    (ContextUse::reported(76_000, 200_000), "38%"),
    (ContextUse::reported(172_000, 200_000), "86%"),
    (ContextUse::reported(200_000, 200_000), "Full"),
  ];
  for (reading, share) in turns {
    turn(&window, &seeded, &reading);
    window.until(&format!("{share} in both meters"), |window| {
      [COMPOSER, step.as_str()]
        .iter()
        .all(|meter| window.texts(&format!("{meter}-share")) == Some(vec![share.to_owned()]))
    });
  }
  click(&mut window, COMPOSER);
  window.until("the popover", |window| {
    window.texts(&format!("{COMPOSER}-popover-tokens")).is_some()
  });
  assert_eq!(
    window.texts(&format!("{COMPOSER}-popover-tokens")),
    Some(vec!["200k of 200k tokens".to_owned()])
  );
  assert_eq!(
    window.texts(&format!("{COMPOSER}-popover-badge-label")),
    Some(vec!["Full".to_owned()])
  );
  turn(&window, &seeded, &ContextUse::new(108_000, 128_000, true));
  window.until("the open popover follows the next turn", |window| {
    window.texts(&format!("{COMPOSER}-popover-tokens")) == Some(vec!["≈ 108k of 128k tokens".to_owned()])
  });
  assert!(
    window
      .tree
      .get_element_by_id(&format!("{COMPOSER}-popover-note"))
      .is_some(),
    "an estimate says it is one"
  );
}
