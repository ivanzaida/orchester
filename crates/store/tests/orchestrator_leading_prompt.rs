//! OA-18: the prompt of a turn an event started by itself carries the leading note only when the subscription the
//! event came through leads its task to the end.
//! `cargo test -p orchester-store --test orchestrator_leading_prompt leading_prompt`.

mod support;

use orchester_core::{
  id::ChatId,
  orchestrator::{AgentWait, Ceilings, Chat, RunEnd, Subscription, WaitKind, WatchMode, WatchTarget},
};
use support::{at, started_run};

#[test]
fn leading_prompt_is_given_only_through_a_subscription_that_leads_to_the_end() {
  let (_repo, mut db, run, _execution) = started_run();
  let wait = AgentWait {
    kind: WaitKind::Ended(RunEnd::Failed),
    task: "ORC-1".to_owned(),
    title: "Add login".to_owned(),
    run: run.id(),
    number: run.number().get(),
    step: None,
    place: None,
    others: 0,
    agent: None,
    link: "/project/p/tasks/ORC-1/summary".to_owned(),
    automatic: true,
    continuing: None,
    merging: None,
  };
  let following = Chat::named(ChatId::generate(), "Following", at(10));
  db.keep_watch_entries(&following, &[], at(10)).expect("conversation");
  db.start_subscription(&Subscription::new(following.id(), WatchTarget::Project, "", at(11)))
    .expect("follows");
  let followed = db.wait_turn_prompt(following.id(), &wait).expect("prompt");
  assert!(!followed.contains("leads this task to the end"), "{followed}");

  // Another conversation leads the task itself to the end: the more specific subscription takes its events.
  let leading = Chat::named(ChatId::generate(), "Leading", at(20));
  db.keep_watch_entries(&leading, &[], at(20)).expect("conversation");
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  db.start_subscription(&Subscription::new(leading.id(), WatchTarget::Task(run.task()), "", at(21)).in_mode(lead))
    .expect("leads");
  let led = db.wait_turn_prompt(leading.id(), &wait).expect("prompt");
  assert!(led.contains("leads this task to the end"), "{led}");
  assert!(
    led.contains("run the task again with run_task"),
    "what to do about a failure: {led}"
  );
}
