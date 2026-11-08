//! The Orchestrator chat and the waits of its project's agents (ORC-42): what a turn a wait started tells the agent;
//! and the system prompt's rules for answering agents, merging runs and where events come (only to the conversation
//! that leads their task, ORC-895).

use orchester_core::{
  id::{ChatMessageId, ProjectId, RunId},
  orchestrator::{
    AgentWait, EntryKind, WaitKind,
    instructions::{ProjectReads, system_prompt, wait_prompt},
  },
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
    place: None,
    others: 0,
    link: "/project/p/tasks/KON-12/conversation".to_owned(),
    automatic: false,
    continuing: None,
    merging: None,
  }
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
    wait_prompt(&wait(run, WaitKind::Plan)).contains("a plan's decision follows decide_plan's rules"),
    "a plan follows decide_plan's rules"
  );
}

#[test]
fn orchestrator_system_prompt_answers_only_what_follows_plainly_and_merges_only_what_was_asked() {
  let prompt = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
  for rule in [
    "get_conversation",
    "only when the answer follows plainly from the task's brief and criteria, the project's docs and AGENTS.md, or \
     what the person already said in this chat",
    "Otherwise tell the person what the agent asks, with its options and your recommendation",
    "Let the Orchestrator drive",
    "merge a run when the person asked for its merge",
    "needs_decision or merge_failed",
    "Always tell the person what you answered, sent, decided or merged, with its link.",
    "Approving tool requests (decide_approval) and deciding plans (decide_plan) follow those tools' rules",
    "an event of a task no conversation leads comes to no conversation",
  ] {
    assert!(prompt.contains(rule), "the system prompt says: {rule}");
  }
}
