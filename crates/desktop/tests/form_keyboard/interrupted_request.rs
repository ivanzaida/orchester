//! A tool request that ended without the person's decision because the agent's permission prompt closed
//! (run 26 of ORC-21), from activity seeded as the runner records it: the problem row says what happened in
//! words that stay whole at 960 and names the request under them, the turn that failed with it is not a
//! second, nameless error row, the `Grep` the CLI denied reads as a denied row, and *Show the request*
//! scrolls the chat back to the request's card.

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
  narrow_answers::{details, inside, open_conversation},
  native_run::native_run,
  task_run::has,
};

const COMMAND: &str = "cargo metadata --format-version 1 --no-deps | python -c \"import json,sys;                        print(json.load(sys.stdin)['target_directory'])\"";
const TITLE: &str = "Request ended without your decision";
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

/// The project's run open in Conversation at 1440 × 960, the problem row shown; the ids of its link and of
/// the request's card, and the row's sequence.
fn opened() -> (Desktop, String, String, u64) {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| seeded = Some(interrupted(repo)));
  let (request, problem) = seeded.expect("seeded");
  desktop.tree.resize(1440, 960);
  open_conversation(&mut desktop);
  let link = format!("activity-show-request-{problem}");
  desktop.wait_for(&link);
  (desktop, link, format!("conversation-approval-{request}"), problem)
}

/// Presses `link` with the card out of the chat's view, and the card then shows in it.
fn reaches(desktop: &mut Desktop, link: &str, card: &str) {
  assert!(!has(desktop, card), "the card is out of the chat's view");
  desktop.click(link);
  // The list may mount the card above its view before it scrolls to it.
  desktop.wait_until("the card in the chat's view", |desktop| {
    has(desktop, card) && {
      let (shown, chat) = (desktop.bounds(card), desktop.bounds("conversation-main"));
      shown.height > 0.0 && inside(shown, chat)
    }
  });
}

/// *Jump to latest* puts the card out of the chat's view again.
fn jump(desktop: &mut Desktop, card: &str) {
  desktop.click("conversation-jump");
  desktop.wait_until("the chat at its end", |desktop| !has(desktop, card));
}

#[test]
fn interrupted_request_is_named_and_its_link_scrolls_to_its_card() {
  let (mut desktop, link, card, problem) = opened();
  let title = format!("activity-entry-title-{problem}");
  assert!(shows(&mut desktop, &title, TITLE));
  assert!(shows(
    &mut desktop,
    &format!("activity-entry-details-{problem}"),
    COMMAND
  ));
  assert!(shows(
    &mut desktop,
    &format!("activity-entry-meta-{problem}"),
    "decision_interrupted"
  ));
  // What happened stays whole at 960, with Details closed and open: the meta gives way first.
  let whole = desktop.bounds(&title).width;
  desktop.tree.resize(960, 960);
  for open in [false, true] {
    details(&mut desktop, open);
    desktop.pass();
    let narrow = desktop.bounds(&title).width;
    assert!(narrow >= whole - 0.5, "Details open: {open}: {narrow} of {whole}");
  }
  details(&mut desktop, false);
  desktop.tree.resize(1440, 960);
  desktop.pass();
  desktop.wait_until("the link laid out", |desktop| {
    desktop
      .tree
      .get_element_by_id_mut(&link)
      .and_then(|element| element.bounds())
      .is_some()
  });
  assert!(
    desktop.bounds(&link).height >= 22.0 - 0.5,
    "the link is as easy to hit as a quiet action"
  );
  assert!(shows(&mut desktop, "conversation", "Grep lurq_[a-z_]+"));
  assert!(
    !shows(&mut desktop, "conversation", "The agent CLI reported an error"),
    "the failed turn is the named request's row, not a second one"
  );
  reaches(&mut desktop, &link, &card);
}

#[test]
fn interrupted_request_link_reaches_its_card_again_after_jump_to_latest() {
  let (mut desktop, link, card, _) = opened();
  reaches(&mut desktop, &link, &card);
  jump(&mut desktop, &card);
  reaches(&mut desktop, &link, &card);
  // Jumping in Tools, then back in All.
  desktop.click("conversation-show-2");
  jump(&mut desktop, &card);
  desktop.click("conversation-show-0");
  desktop.wait_for(&link);
  reaches(&mut desktop, &link, &card);
}

#[test]
fn interrupted_request_link_in_tools_shows_the_card_in_all() {
  let (mut desktop, link, card, _) = opened();
  // *Tools* lists the problem as an entry and no card: the link brings back *All* with the card.
  desktop.click("conversation-show-2");
  desktop.wait_for(&link);
  reaches(&mut desktop, &link, &card);
}
