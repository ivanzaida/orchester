//! The Orchestrator chat follows the ends of its project's runs and proposes how a stopped run goes on
//! (`docs/design/orchestrator-toolset.md`, decision 5): an end or a wait the chat's own doing led to (its
//! continuation, answer or message, its resume of the run; for an end its start of the run) starts no turn by itself,
//! so the chat never takes up its own doing in a loop, but a run that succeeded always may; the prompt of an end's
//! turn says what to do with each end, inviting a retry or a return only for a run that waits for a decision; and the
//! system prompt names what the chat may propose for a stopped run and what a turn nobody asked for only proposes.

use orchester_core::{
  id::{ProjectId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, RunEnd, StopReason, WaitKind,
    change::{Change, ContinueChoice, ProposedChange, StepName},
    instructions::{system_prompt, wait_prompt},
    wait::raised_by_own,
  },
  run::RunLimit,
  time::Timestamp,
};

const STOPPED: RunEnd = RunEnd::Stopped {
  why: StopReason::RetriesExhausted { retries: 2 },
};

fn ended(run: RunId, end: RunEnd) -> AgentWait {
  AgentWait {
    kind: WaitKind::Ended(end),
    task: "KON-12".to_owned(),
    title: "Sync contacts".to_owned(),
    run,
    number: 3,
    step: Some("Review".to_owned()),
    place: Some(4),
    others: 0,
    agent: None,
    link: "/project/p/tasks/KON-12/summary".to_owned(),
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

fn kept(seq: i64, wait: &AgentWait) -> ChatEntry {
  entry(
    seq,
    EntryKind::Wait {
      wait: wait.clone(),
      turn: None,
    },
  )
}

/// The chat's proposal to continue `run` with one more retry, gone as `outcome`.
fn continued(run: RunId, outcome: ActionOutcome) -> EntryKind {
  EntryKind::Action(ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Sync contacts".to_owned(),
      change: Change::Continue {
        run,
        number: 3,
        attempt: 2,
        step: Some(StepName {
          place: 4,
          name: "Review".to_owned(),
        }),
        why: Some(StopReason::RetriesExhausted { retries: 2 }),
        choice: ContinueChoice::AllowRetry,
      },
    })),
    ..ChatAction::new("continue_run", Some("KON-12".to_owned()), outcome)
  })
}

/// The chat's message to `run`'s agents, sent.
fn messaged(run: RunId) -> EntryKind {
  EntryKind::Action(ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Sync contacts".to_owned(),
      change: Change::Message {
        run,
        number: 3,
        text: "Keep the old export.".to_owned(),
        now: false,
      },
    })),
    ..ChatAction::new("send_message", Some("KON-12".to_owned()), ActionOutcome::Done)
  })
}

#[test]
fn orchestrator_run_end_after_the_chats_own_decision_is_its_own() {
  let run = RunId::generate();
  let stopped = ended(run, STOPPED);
  let failed = ended(run, RunEnd::Failed);
  assert!(
    !raised_by_own(std::slice::from_ref(&kept(1, &stopped)), &stopped),
    "the run stopped on its own"
  );
  let applied = [kept(1, &stopped), entry(2, continued(run, ActionOutcome::Applied))];
  assert!(
    raised_by_own(&applied, &failed),
    "the person applied the chat's retry, and the run failed after it"
  );
  let proposed = [kept(1, &stopped), entry(2, continued(run, ActionOutcome::Waits))];
  assert!(!raised_by_own(&proposed, &failed), "a proposal changed nothing");
  let dismissed = [kept(1, &stopped), entry(2, continued(run, ActionOutcome::Dismissed))];
  assert!(
    !raised_by_own(&dismissed, &failed),
    "a dismissed proposal changed nothing"
  );
  let other = RunId::generate();
  assert!(
    !raised_by_own(&applied, &ended(other, RunEnd::Failed)),
    "another run's decision raises nothing here"
  );
}

#[test]
fn orchestrator_run_end_after_the_chats_resume_or_start_of_that_run_is_its_own() {
  let run = RunId::generate();
  let resumed = |of: RunId| {
    entry(
      1,
      EntryKind::Action(ChatAction {
        resumed: Some(of),
        ..ChatAction::new("resume_run", Some("KON-12".to_owned()), ActionOutcome::Done)
      }),
    )
  };
  let limited = ended(run, RunEnd::Limit { limit: RunLimit::Time });
  assert!(raised_by_own(&[resumed(run)], &limited), "the chat resumed this run");
  assert!(
    !raised_by_own(&[resumed(RunId::generate())], &limited),
    "a resume of another run of the task is not this run's"
  );
  assert!(
    !raised_by_own(&[resumed(run), kept(2, &ended(run, STOPPED))], &limited),
    "a resume suppresses only the run's next wait"
  );
  let started = [entry(
    1,
    EntryKind::Action(ChatAction {
      started: Some(run),
      auto_mode: true,
      ..ChatAction::new("run_task", Some("KON-12".to_owned()), ActionOutcome::Done)
    }),
  )];
  assert!(
    raised_by_own(&started, &ended(run, RunEnd::Failed)),
    "the chat started it"
  );
  let mut question = ended(run, STOPPED);
  question.kind = WaitKind::Question;
  assert!(
    !raised_by_own(&started, &question),
    "an agent's question in a run the chat started is not the chat's doing"
  );
}

#[test]
fn orchestrator_run_that_succeeded_is_never_the_chats_own() {
  let run = RunId::generate();
  let ready = ended(run, RunEnd::Succeeded);
  for before in [continued(run, ActionOutcome::Applied), messaged(run)] {
    assert!(
      !raised_by_own(&[entry(1, before)], &ready),
      "a success turn cannot loop"
    );
  }
}

#[test]
fn orchestrator_run_end_is_kept_with_how_it_ended() {
  let end = ended(
    RunId::generate(),
    RunEnd::Limit {
      limit: RunLimit::DailyBudget,
    },
  );
  let end = AgentWait { others: 2, ..end };
  let kept = serde_json::to_string(&end).expect("serialized");
  assert!(
    kept.contains(r#""ended":{"end":"limit","limit":"daily_budget"}"#),
    "{kept}"
  );
  assert!(kept.contains(r#""others":2"#), "{kept}");
  let read: AgentWait = serde_json::from_str(&kept).expect("read back");
  assert_eq!(read, end);
  let stopped = serde_json::to_string(&ended(RunId::generate(), STOPPED)).expect("serialized");
  assert!(
    stopped.contains(r#""ended":{"end":"stopped","why":{"reason":"retries_exhausted","retries":2}}"#),
    "{stopped}"
  );
  let question = AgentWait {
    kind: WaitKind::Question,
    ..read
  };
  assert_eq!(question.kind.run_end(), None, "an agent's wait is no run's end");
}

#[test]
fn orchestrator_run_end_prompt_says_what_to_do_with_each_end() {
  let run = RunId::generate();
  let ready = wait_prompt(&ended(run, RunEnd::Succeeded));
  for part in [
    "Run #3 of KON-12 (Sync contacts) succeeded: its work is ready to merge",
    "/project/p/tasks/KON-12/summary",
    "Let the Orchestrator answer agents is on",
    "If the person asked you earlier in this conversation",
    "in this turn it only proposes the merge",
    "Never merge a run nobody asked you to merge",
  ] {
    assert!(ready.contains(part), "{part} in {ready}");
  }
  let stopped = wait_prompt(&ended(run, STOPPED));
  for part in [
    "stopped at the step Review (its step used up its 2 retries) and waits for a decision",
    "continue_run (allow_retry or return_to)",
    "the person applies it",
    "Start, resume, cancel or merge nothing",
  ] {
    assert!(stopped.contains(part), "{part} in {stopped}");
  }
  for (end, says) in [
    (RunEnd::Failed, "failed at the step Review"),
    (
      RunEnd::Limit {
        limit: RunLimit::DailyBudget,
      },
      "stopped at the project's daily token budget",
    ),
    (RunEnd::Interrupted, "was interrupted"),
  ] {
    let prompt = wait_prompt(&ended(run, end));
    for part in [
      says,
      "Read the run first",
      "the choices they have on the task's Summary",
      "are the person's, and continue_run offers you nothing for this run",
      "Start, resume, cancel, continue or merge nothing",
    ] {
      assert!(prompt.contains(part), "{end:?}: {part} in {prompt}");
    }
    assert!(
      !prompt.contains("allow_retry") && !prompt.contains("merge_run"),
      "{end:?} invites no retry and no merge: {prompt}"
    );
  }
  let batch = AgentWait {
    others: 2,
    ..ended(
      run,
      RunEnd::Limit {
        limit: RunLimit::DailyBudget,
      },
    )
  };
  assert!(
    wait_prompt(&batch).contains("The same limit stopped 2 more runs of the project at once."),
    "{}",
    wait_prompt(&batch)
  );
}

#[test]
fn orchestrator_system_prompt_proposes_only_a_retry_or_a_return() {
  let prompt = system_prompt("Kontur", ProjectId::generate());
  for rule in [
    "how a stopped run continues",
    "continue_run proposes how a run that waits for a decision goes on: allow_retry",
    "return_to (an earlier agent step, with a note for its agent)",
    "Raising limits or budgets, resuming after the daily budget and accepting a step are the person's",
    "In a turn Orchester started by itself (its message begins with [Orchester]), merge_run and run_task only \
     propose",
  ] {
    assert!(prompt.contains(rule), "the system prompt says: {rule}");
  }
}
