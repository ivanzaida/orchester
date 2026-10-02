//! Resuming a native conversation, and compacting it when it nears the context window.

use orchester_core::{
  connection::stream::{FileChange, FileChangeKind},
  native::{
    command::CommandPolicy,
    item::{Item, UserOrigin},
    model::{ModelError, StopReason},
    session::{
      COMPACTION_REQUEST, Continuation, LoopAction, LoopEnd, LoopError, LoopFailure, LoopInput, MAX_COMPACTIONS,
      NOT_RUN, NativeLoop,
    },
  },
  permission::{CommandPermission, RepositoryAccess},
};
use serde_json::json;

use crate::support::{
  Layout, assistant, call, ended, ok, persisted, request, response, settings, start_with, tool_call,
};

fn task() -> Item {
  Item::user("Do the task.", UserOrigin::Task)
}

fn resume(
  layout: &Layout,
  stored: Vec<Item>,
  continuation: Continuation,
) -> Result<(NativeLoop, Vec<LoopAction>), LoopError> {
  let policy = CommandPolicy::new(&CommandPermission::AskBefore).expect("policy");
  NativeLoop::resume(
    settings(RepositoryAccess::Write, 200_000),
    layout.scope(RepositoryAccess::Write),
    policy,
    stored,
    continuation,
  )
}

#[test]
fn native_resume_answers_unanswered_calls() {
  let layout = Layout::new();
  let stored = vec![
    task(),
    assistant("Reading."),
    tool_call("r1", "read_file", json!({"path": "src/main.rs"})),
    tool_call("c1", "run_command", json!({"command": "cargo test"})),
    Item::tool_result(call("r1"), "fn main() {}", false),
  ];
  let (_, actions) = resume(&layout, stored.clone(), Continuation::Prompt("Continue.".to_owned())).expect("resume");
  let appended = vec![
    Item::tool_result(call("c1"), NOT_RUN, true),
    Item::user("Continue.", UserOrigin::Resume),
  ];
  assert_eq!(persisted(&actions), appended);
  let mut expected = stored;
  expected.extend(appended);
  assert_eq!(request(&actions).expect("request").items, expected);
}

#[test]
fn native_resume_with_the_answer_to_a_question() {
  let layout = Layout::new();
  let stored = vec![
    task(),
    tool_call("q1", "ask_question", json!({"question": "Which database?"})),
  ];
  let (_, actions) = resume(&layout, stored, Continuation::Answer("SQLite.".to_owned())).expect("resume");
  assert_eq!(
    persisted(&actions),
    vec![Item::tool_result(call("q1"), "SQLite.", false)]
  );

  let (_, actions) = resume(
    &layout,
    vec![task(), assistant("Done?")],
    Continuation::Answer("Yes.".to_owned()),
  )
  .expect("resume");
  assert_eq!(persisted(&actions), vec![Item::user("Yes.", UserOrigin::Answer)]);
}

#[test]
fn native_resume_continues_the_latest_segment() {
  let layout = Layout::new();
  let note = Item::user("Earlier turns were replaced.", UserOrigin::Compaction);
  let compaction = Item::Compaction {
    summary: "So far: nothing.".to_owned(),
  };
  let stored = vec![
    task(),
    assistant("old"),
    compaction.clone(),
    note.clone(),
    assistant("new"),
  ];
  let (_, actions) = resume(&layout, stored, Continuation::Prompt("Continue.".to_owned())).expect("resume");
  let items = &request(&actions).expect("request").items;
  assert_eq!(items[..4], [task(), compaction, note, assistant("new")]);

  assert!(matches!(
    resume(
      &layout,
      vec![assistant("no task")],
      Continuation::Prompt("x".to_owned())
    ),
    Err(LoopError::NoTask)
  ));
}

#[test]
fn native_compaction_near_the_window() {
  let layout = Layout::new();
  let (mut native, _) = start_with(
    &layout,
    RepositoryAccess::Write,
    settings(RepositoryAccess::Write, 3000),
    &[],
  );
  let edit = tool_call(
    "e1",
    "edit_file",
    json!({"path": "src/main.rs", "edits": [{"old": "fn", "new": "pub fn"}]}),
  );
  native
    .handle(LoopInput::Responded(response(vec![edit], StopReason::ToolUse, 2900)))
    .expect("response");
  let mut outcome = ok("edited");
  outcome.changes = vec![FileChange {
    path: "src/main.rs".to_owned(),
    kind: FileChangeKind::Modified,
  }];
  let actions = native.handle(LoopInput::ToolDone(call("e1"), outcome)).expect("e1");
  let fork = request(&actions).expect("summary request");
  assert_eq!(
    fork.items.last(),
    Some(&Item::user(COMPACTION_REQUEST, UserOrigin::Compaction))
  );
  assert!(persisted(&actions).iter().all(|item| !matches!(
    item,
    Item::User {
      origin: UserOrigin::Compaction,
      ..
    }
  )));

  let summary = response(vec![assistant("Made main public.")], StopReason::EndTurn, 950);
  let actions = native.handle(LoopInput::Responded(summary)).expect("summary");
  let stored = persisted(&actions);
  assert_eq!(
    stored[0],
    Item::Compaction {
      summary: "Made main public.".to_owned()
    }
  );
  let Item::User { text: note, .. } = &stored[1] else {
    panic!("note")
  };
  assert!(note.contains("src/main.rs"), "{note}");
  assert_eq!(
    request(&actions).expect("next request").items,
    [vec![task()], stored].concat()
  );
}

#[test]
fn native_compaction_is_limited() {
  let layout = Layout::new();
  let (mut native, _) = start_with(
    &layout,
    RepositoryAccess::Write,
    settings(RepositoryAccess::Write, 10),
    &[],
  );
  let mut actions = Vec::new();
  for _ in 0..MAX_COMPACTIONS {
    let summary = response(vec![assistant("Summary.")], StopReason::EndTurn, 5);
    actions = native.handle(LoopInput::Responded(summary)).expect("summary");
  }
  assert_eq!(ended(&actions), Some(&LoopEnd::Failed(LoopFailure::ContextExhausted)));
}

#[test]
fn native_compaction_when_the_provider_says_so() {
  let layout = Layout::new();
  let (mut native, _) = start_with(
    &layout,
    RepositoryAccess::Write,
    settings(RepositoryAccess::Write, 200_000),
    &[],
  );
  let actions = native
    .handle(LoopInput::ModelFailed(ModelError::TooLarge))
    .expect("too large");
  let fork = request(&actions).expect("summary request");
  assert_eq!(
    fork.items.last(),
    Some(&Item::user(COMPACTION_REQUEST, UserOrigin::Compaction))
  );
  let actions = native
    .handle(LoopInput::ModelFailed(ModelError::TooLarge))
    .expect("too large again");
  assert_eq!(ended(&actions), Some(&LoopEnd::Failed(LoopFailure::ContextExhausted)));

  let (mut native, _) = start_with(
    &layout,
    RepositoryAccess::Write,
    settings(RepositoryAccess::Write, 200_000),
    &[],
  );
  let full = response(vec![assistant("cut")], StopReason::ContextExceeded, 10);
  let actions = native.handle(LoopInput::Responded(full)).expect("context exceeded");
  let fork = request(&actions).expect("summary request");
  assert_eq!(
    fork.items.last(),
    Some(&Item::user(COMPACTION_REQUEST, UserOrigin::Compaction))
  );
}
