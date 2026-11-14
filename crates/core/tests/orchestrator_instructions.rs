//! OA-18: what a turn of a subscription that leads to the end is told beside its event's prompt, by the kind of the
//! event and, for a stall, by its period; and that an event's own prompt says none of it.
//! OA-07: what a turn that takes several events is told.
//! `cargo test -p orchester-core --test orchestrator_instructions leading together`.

use orchester_core::{
  id::RunId,
  orchestrator::{
    AgentWait, RunEnd, TaskEvent, TaskEventKind, WaitKind,
    instructions::{TOGETHER_MOST, leading_note, together_prompt, wait_prompt},
  },
  time::Timestamp,
};

fn wait(kind: WaitKind) -> AgentWait {
  AgentWait {
    kind,
    task: "KON-12".to_owned(),
    title: "Sync contacts".to_owned(),
    run: RunId::generate(),
    number: 3,
    step: Some("Implement".to_owned()),
    agent: Some("Builder".to_owned()),
    place: None,
    others: 0,
    link: "/project/p/tasks/KON-12/conversation".to_owned(),
    automatic: true,
    continuing: None,
    merging: None,
  }
}

fn stall() -> WaitKind {
  WaitKind::Event(TaskEvent {
    kind: TaskEventKind::NoProgress {
      since: Timestamp::from_unix_millis(0),
      minutes: 20,
    },
    at: Timestamp::from_unix_millis(1_200_000),
  })
}

#[test]
fn leading_note_asks_every_turn_to_move_the_work_and_not_to_report() {
  let note = leading_note(&wait(WaitKind::Question), None);
  for said in [
    "nobody is at the computer",
    "get_standing",
    "what the design shows; what the app already does",
    "park_task",
    "cannot be checked at all",
    "Never wait in words",
    "Give your reason",
  ] {
    assert!(note.contains(said), "{said}: {note}");
  }
  assert!(
    !note.contains("About this"),
    "a question has no part of its own: {note}"
  );
}

#[test]
fn leading_note_says_what_to_do_about_a_request_and_a_failure() {
  let request = leading_note(&wait(WaitKind::Approval), None);
  assert!(request.contains("park the task as an approval"), "{request}");
  assert!(request.contains("park rather than deny again"), "{request}");
  let failure = leading_note(&wait(WaitKind::Ended(RunEnd::Failed)), None);
  assert!(failure.contains("continue_run return_to"), "{failure}");
  assert!(failure.contains("run the task again with run_task"), "{failure}");
  assert!(failure.contains("park rather than repeat the note"), "{failure}");
  let success = leading_note(&wait(WaitKind::Ended(RunEnd::Succeeded)), None);
  assert!(!success.contains("About this"), "{success}");
}

#[test]
fn leading_note_of_a_stall_acts_in_steps() {
  let first = leading_note(&wait(stall()), Some(1));
  assert!(first.contains("send_message"), "{first}");
  let second = leading_note(&wait(stall()), Some(2));
  assert!(
    second.contains("pause_run") && second.contains("resume_run"),
    "{second}"
  );
  let third = leading_note(&wait(stall()), Some(3));
  assert!(third.contains("Park the task"), "{third}");
  assert_eq!(leading_note(&wait(stall()), Some(7)), third, "no step after the third");
  assert_eq!(
    leading_note(&wait(stall()), None),
    first,
    "a period nobody counted is the first"
  );
}

#[test]
fn leading_note_is_not_part_of_an_events_own_prompt() {
  for kind in [
    WaitKind::Question,
    WaitKind::Approval,
    WaitKind::Ended(RunEnd::Failed),
    stall(),
  ] {
    let prompt = wait_prompt(&wait(kind));
    assert!(!prompt.contains("leads this task to the end"), "{prompt}");
  }
}

#[test]
fn together_prompt_tells_each_event_in_order_and_one_event_as_it_is() {
  let question = wait_prompt(&wait(WaitKind::Question));
  assert_eq!(together_prompt(std::slice::from_ref(&question)), question);
  let told = together_prompt(&[question.clone(), wait_prompt(&wait(WaitKind::Plan))]);
  assert!(
    told.starts_with("[Orchester] 2 events wait in this conversation"),
    "{told}"
  );
  let (first, second) = (
    told.find("Event 1 of 2:").expect("the first"),
    told.find("Event 2 of 2:").expect("the second"),
  );
  assert!(first < second && told[first..second].contains(&question), "{told}");
  assert!(told.contains("stays open"), "{told}");
  assert_eq!(TOGETHER_MOST, 5);
}
