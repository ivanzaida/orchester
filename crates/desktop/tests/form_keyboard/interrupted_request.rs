//! A tool request that ended without the person's decision because the agent's permission prompt closed
//! (run 26 of ORC-21), from activity seeded as the runner records it: the problem row names the request,
//! the turn that failed with it is not a second, nameless error row, the `Grep` the CLI denied reads as a
//! denied row, and *Show the request* scrolls the chat back to the request's card.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, RunnerProblem},
  run::PendingToolApproval,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  narrow_answers::{inside, open_conversation},
  native_run::native_run,
  task_run::has,
};

const COMMAND: &str = "cargo metadata --format-version 1 --no-deps";
/// Messages between the request and the problem row: enough to put the card out of the chat's view.
const MESSAGES: usize = 30;

/// The request the agent waited on and the interrupted end of its turn; the request's id and the
/// problem row's sequence.
fn interrupted(repo: &std::path::Path) -> (String, u64) {
  let (run, invocation) = native_run(repo);
  let mut db = ProjectDb::open(repo).expect("open the project");
  let at = |second: i64| Timestamp::from_unix_millis(20_000 + second * 1_000);
  let request = PendingToolApproval {
    id: format!("{}:toolu_meta", invocation.id),
    run: run.id(),
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: COMMAND.to_owned(),
    target: None,
    requested_at: at(1),
    native: None,
  };
  db.request_tool_approval(&request).expect("the request");
  let mut events = vec![AgentEvent::CommandStarted {
    item: "toolu_meta".into(),
    command: COMMAND.into(),
  }];
  events.extend((0..MESSAGES).map(|index| AgentEvent::Message {
    text: format!("Still waiting for the approval, check {index}."),
  }));
  events.extend([
    AgentEvent::CommandFinished {
      item: "toolu_meta".into(),
      command: COMMAND.into(),
      output: "MCP server Orchester tool approve sent no response or progress for 1800s; aborting.".into(),
      exit_code: None,
      status: CommandStatus::Failed,
    },
    AgentEvent::PermissionDenied {
      item: "toolu_grep".into(),
      command: "Grep lurq_[a-z_]+".into(),
    },
    AgentEvent::RunnerProblem {
      problem: RunnerProblem::DecisionInterrupted {
        request: request.id.clone(),
        server: "command".into(),
        action: COMMAND.into(),
      },
    },
    AgentEvent::TurnFailed {
      message: "Claude Code's permission prompt or tool relay closed before the decision".into(),
    },
  ]);
  let events: Vec<_> = events
    .into_iter()
    .enumerate()
    .map(|(index, event)| (at(2 + index as i64), event))
    .collect();
  let last = db.append_activity(&invocation, &events).expect("store the events");
  db.interrupt_tool_approvals(run.id(), invocation.id)
    .expect("interrupt the request");
  (request.id, last - 1)
}

#[test]
fn interrupted_request_is_named_and_its_link_scrolls_to_its_card() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| seeded = Some(interrupted(repo)));
  let (request, problem) = seeded.expect("seeded");
  desktop.tree.resize(1440, 960);
  open_conversation(&mut desktop);
  let link = format!("activity-show-request-{problem}");
  desktop.wait_for(&link);
  let row = format!("activity-entry-title-{problem}");
  assert!(shows(
    &mut desktop,
    &row,
    &format!("The request for {COMMAND} ended without your decision")
  ));
  assert!(shows(&mut desktop, "conversation", "Grep lurq_[a-z_]+"));
  assert!(
    !shows(&mut desktop, "conversation", "The agent CLI reported an error"),
    "the failed turn is the named request's row, not a second one"
  );
  let card = format!("conversation-approval-{request}");
  assert!(!has(&mut desktop, &card), "the card is out of the chat's view");
  desktop.click(&link);
  desktop.wait_for(&card);
  let (card, chat) = (desktop.bounds(&card), desktop.bounds("conversation-main"));
  assert!(card.height > 0.0 && inside(card, chat), "{card:?} shows in {chat:?}");
}
