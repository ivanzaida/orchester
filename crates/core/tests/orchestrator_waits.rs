//! The Orchestrator chat and the waits of its project's agents (ORC-42): a wait the Orchestrator's own answer raised
//! starts no turn by itself, so the chat and an agent never answer each other in a loop; what a turn a wait started
//! tells the agent; and the system prompt's rules for answering agents and merging runs.

use orchester_core::{
  id::{ChatMessageId, ProjectId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, WaitKind,
    change::{Change, ProposedChange},
    instructions::{system_prompt, wait_prompt},
    wait::raised_by_own,
  },
  time::Timestamp,
};

fn wait(run: RunId, kind: WaitKind) -> AgentWait {
  AgentWait {
    kind,
    task: "KON-12".to_owned(),
    title: "Sync contacts".to_owned(),
    run,
    number: 3,
    step: Some("Implement".to_owned()),
    agent: Some("Builder".to_owned()),
    link: "/project/p/tasks/KON-12/conversation".to_owned(),
    own: false,
    automatic: false,
  }
}

fn entry(seq: i64, kind: EntryKind) -> ChatEntry {
  ChatEntry {
    seq,
    at: Timestamp::from_unix_millis(seq * 10),
    kind,
  }
}

/// The chat's answer to `run` (a message when `message`), gone as `outcome`.
fn answered(run: RunId, message: bool, outcome: ActionOutcome) -> EntryKind {
  let change = if message {
    Change::Message {
      run,
      number: 3,
      text: "Keep the old export.".to_owned(),
      now: false,
    }
  } else {
    Change::Answer {
      run,
      number: 3,
      request: "ask-1".to_owned(),
      choices: vec!["Sidebar".to_owned()],
      text: None,
    }
  };
  let tool = if message { "send_message" } else { "answer_question" };
  EntryKind::Action(ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Sync contacts".to_owned(),
      change,
    })),
    ..ChatAction::new(tool, Some("KON-12".to_owned()), outcome)
  })
}

#[test]
fn orchestrator_wait_raised_by_its_own_answer_is_known() {
  let run = RunId::generate();
  assert!(!raised_by_own(&[], run), "nothing answered yet");
  let sent = [entry(1, answered(run, true, ActionOutcome::Done))];
  assert!(raised_by_own(&sent, run), "the chat sent the run's agents a message itself");
  let applied = [entry(1, answered(run, false, ActionOutcome::Applied))];
  assert!(raised_by_own(&applied, run), "the person applied the chat's answer");
  let proposed = [entry(1, answered(run, false, ActionOutcome::Waits))];
  assert!(!raised_by_own(&proposed, run), "a proposal sent nothing");
  let other = RunId::generate();
  assert!(!raised_by_own(&sent, other), "another run's answer raises nothing here");
}

#[test]
fn orchestrator_wait_after_a_newer_wait_of_the_run_is_not_its_own() {
  let run = RunId::generate();
  let entries = [
    entry(1, answered(run, true, ActionOutcome::Done)),
    entry(
      2,
      EntryKind::Wait {
        wait: wait(run, WaitKind::Message),
        turn: None,
      },
    ),
  ];
  assert!(
    !raised_by_own(&entries, run),
    "the agent asked again after the chat's answer; the next wait follows that one, not the answer"
  );
}

#[test]
fn orchestrator_wait_turn_is_one_of_a_wait() {
  let run = RunId::generate();
  let message = ChatMessageId::generate();
  let started = EntryKind::Wait {
    wait: wait(run, WaitKind::Question),
    turn: Some(orchester_core::orchestrator::WaitTurn {
      message,
      state: orchester_core::orchestrator::TurnState::Thinking,
      ended: None,
    }),
  };
  assert_eq!(started.turn().map(|(id, _)| id), Some(message));
  let event = EntryKind::Wait {
    wait: wait(run, WaitKind::Question),
    turn: None,
  };
  assert!(event.turn().is_none(), "a wait that started no turn is no turn");
}

#[test]
fn orchestrator_wait_prompt_names_the_wait_and_the_rules() {
  let run = RunId::generate();
  for (kind, says) in [
    (WaitKind::Question, "asks a question with suggested answers"),
    (WaitKind::Message, "waits for a message"),
    (WaitKind::Plan, "plan that waits for a decision"),
  ] {
    let prompt = wait_prompt(&wait(run, kind));
    for part in [
      says,
      "Builder",
      "Implement",
      "run #3 of KON-12",
      "/project/p/tasks/KON-12/conversation",
      "follows plainly",
      "with the link",
    ] {
      assert!(prompt.contains(part), "{kind:?}: {part} in {prompt}");
    }
  }
  assert!(
    wait_prompt(&wait(run, WaitKind::Plan)).contains("a plan's decision is the person's"),
    "a plan stays the person's"
  );
}

#[test]
fn orchestrator_system_prompt_answers_only_what_follows_plainly_and_merges_only_what_was_asked() {
  let prompt = system_prompt("Kontur", ProjectId::generate());
  for rule in [
    "get_conversation",
    "only when the answer follows plainly from the task's brief and criteria, the project's docs and AGENTS.md, or \
     what the person already said in this chat",
    "Otherwise tell the person what the agent asks, with its options and your recommendation",
    "Let the Orchestrator answer agents",
    "Merge a run only when the person asked in this chat for that run's merge",
    "Let the Orchestrator merge runs",
    "needs_decision or merge_failed",
    "Always tell the person what you answered, sent or merged, with its link.",
    "You never approve tool requests, decide plans",
  ] {
    assert!(prompt.contains(rule), "the system prompt says: {rule}");
  }
}
