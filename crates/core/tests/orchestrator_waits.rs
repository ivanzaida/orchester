//! The Orchestrator chat and the waits of its project's agents (ORC-42): a wait the Orchestrator's own answer raised
//! starts no turn by itself, so the chat and an agent never answer each other in a loop; a wait goes to the
//! conversation that last dealt with its task; what a turn a wait started tells the agent; and the system prompt's
//! rules for answering agents and merging runs.

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ProjectId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, TurnState, WaitKind,
    change::{Change, ProposedChange},
    instructions::{ProjectReads, system_prompt, wait_prompt},
    wait::{dealt_with, raised_by_own},
  },
  run::Answerer,
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
    place: None,
    others: 0,
    link: "/project/p/tasks/KON-12/conversation".to_owned(),
    own: false,
    automatic: false,
    continuing: None,
    merging: None,
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
      then: None,
      sent_as: None,
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
  assert!(
    !raised_by_own(&[], &wait(run, WaitKind::Message)),
    "nothing answered yet"
  );
  let sent = [entry(1, answered(run, true, ActionOutcome::Done))];
  assert!(
    raised_by_own(&sent, &wait(run, WaitKind::Message)),
    "the chat sent the run's agents a message itself"
  );
  let applied = [entry(1, answered(run, false, ActionOutcome::Applied))];
  assert!(
    raised_by_own(&applied, &wait(run, WaitKind::Message)),
    "the person applied the chat's answer"
  );
  let proposed = [entry(1, answered(run, false, ActionOutcome::Waits))];
  assert!(
    !raised_by_own(&proposed, &wait(run, WaitKind::Message)),
    "a proposal sent nothing"
  );
  let other = RunId::generate();
  assert!(
    !raised_by_own(&sent, &wait(other, WaitKind::Message)),
    "another run's answer raises nothing here"
  );
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
    !raised_by_own(&entries, &wait(run, WaitKind::Message)),
    "the agent asked again after the chat's answer; the next wait follows that one, not the answer"
  );
}

/// An action of tool `tool` on task `task`, done.
fn acted(tool: &str, task: &str) -> EntryKind {
  EntryKind::Action(ChatAction::new(tool, Some(task.to_owned()), ActionOutcome::Done))
}

fn person(text: &str) -> EntryKind {
  EntryKind::Person {
    message: ChatMessageId::generate(),
    text: MessageBody::new(text).expect("text"),
    turn: TurnState::Answered,
    ended: None,
    sent_by: Answerer::Person,
  }
}

#[test]
fn orchestrator_wait_goes_to_the_conversation_that_last_dealt_with_its_task() {
  let (run, a, b) = (RunId::generate(), ChatId::generate(), ChatId::generate());
  let waiting = wait(run, WaitKind::Question);
  let entries = [
    (a, entry(1, acted("update_task", "kon-12"))),
    (b, entry(2, person("How is KON-12?"))),
    (b, entry(3, acted("create_task", "KON-13"))),
  ];
  let about =
    |entries: &[(ChatId, ChatEntry)]| dealt_with(entries.iter().map(|(chat, entry)| (*chat, entry)), &waiting);
  assert_eq!(
    about(&entries),
    Some(a),
    "the newer conversation only talked about the task and acted on another"
  );
  assert_eq!(about(&entries[1..]), None, "no conversation dealt with it");
  let answered = [
    (a, entry(1, acted("update_task", "KON-12"))),
    (b, entry(2, answered(run, true, ActionOutcome::Dismissed))),
  ];
  assert_eq!(
    about(&answered),
    Some(b),
    "a dismissed message to its run deals with it too"
  );
  let started = ChatAction {
    started: Some(run),
    ..ChatAction::new("run_task", None, ActionOutcome::Done)
  };
  let kept = [
    (b, entry(1, acted("update_task", "KON-12"))),
    (a, entry(2, EntryKind::Action(started))),
    (
      b,
      entry(
        3,
        EntryKind::Wait {
          wait: wait(RunId::generate(), WaitKind::Plan),
          turn: None,
        },
      ),
    ),
  ];
  assert_eq!(about(&kept), Some(b), "a wait of an earlier run of the task kept there");
  assert_eq!(about(&kept[..2]), Some(a), "the conversation that started the run");
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
  let prompt = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
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
