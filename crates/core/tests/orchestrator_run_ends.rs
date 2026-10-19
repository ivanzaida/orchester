//! The Orchestrator chat follows the ends of its project's runs and proposes how a stopped run goes on
//! (`docs/design/orchestrator-toolset.md`, decision 5): an end the chat's own decision on the run led to, or a resume
//! the chat did, starts no turn by itself, so the chat never takes up its own doing in a loop; the prompt of an end's
//! turn says what to do with each end; and the system prompt names what the chat may propose for a stopped run.

use orchester_core::{
  id::{ProjectId, RunId},
  orchestrator::{
    ActionOutcome, AgentWait, ChatAction, ChatEntry, EntryKind, RunEnd, WaitKind,
    change::{Change, ContinueChoice, ProposedChange},
    instructions::{system_prompt, wait_prompt},
    wait::raised_by_own,
  },
  run::RunLimit,
  time::Timestamp,
};

fn ended(run: RunId, end: RunEnd) -> AgentWait {
  AgentWait {
    kind: WaitKind::Ended(end),
    task: "KON-12".to_owned(),
    title: "Sync contacts".to_owned(),
    run,
    number: 3,
    step: Some("Review".to_owned()),
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

/// The chat's proposal to continue `run` with one more retry, gone as `outcome`.
fn continued(run: RunId, outcome: ActionOutcome) -> EntryKind {
  EntryKind::Action(ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Sync contacts".to_owned(),
      change: Change::Continue {
        run,
        number: 3,
        attempt: 2,
        step: Some("Review".to_owned()),
        choice: ContinueChoice::AllowRetry,
      },
    })),
    ..ChatAction::new("continue_run", Some("KON-12".to_owned()), outcome)
  })
}

#[test]
fn orchestrator_run_end_after_the_chats_own_decision_is_its_own() {
  let run = RunId::generate();
  let stopped = ended(run, RunEnd::Stopped);
  let kept = entry(
    1,
    EntryKind::Wait {
      wait: stopped.clone(),
      turn: None,
    },
  );
  assert!(
    !raised_by_own(std::slice::from_ref(&kept), &stopped),
    "the run stopped on its own"
  );
  let applied = [kept.clone(), entry(2, continued(run, ActionOutcome::Applied))];
  let failed = ended(run, RunEnd::Failed);
  assert!(
    raised_by_own(&applied, &failed),
    "the person applied the chat's retry, and the run failed after it"
  );
  let proposed = [kept.clone(), entry(2, continued(run, ActionOutcome::Waits))];
  assert!(!raised_by_own(&proposed, &failed), "a proposal changed nothing");
  let dismissed = [kept, entry(2, continued(run, ActionOutcome::Dismissed))];
  assert!(
    !raised_by_own(&dismissed, &failed),
    "a dismissed proposal changed nothing"
  );
  let resumed = [entry(
    1,
    EntryKind::Action(ChatAction::new(
      "resume_run",
      Some("kon-12".to_owned()),
      ActionOutcome::Done,
    )),
  )];
  assert!(
    raised_by_own(&resumed, &ended(run, RunEnd::Interrupted)),
    "the chat resumed the task's run itself"
  );
  let paused = [entry(
    1,
    EntryKind::Action(ChatAction::new(
      "pause_run",
      Some("KON-12".to_owned()),
      ActionOutcome::Done,
    )),
  )];
  assert!(!raised_by_own(&paused, &failed), "a pause carries nothing on");
}

#[test]
fn orchestrator_run_end_is_kept_with_how_it_ended() {
  let end = ended(
    RunId::generate(),
    RunEnd::Limit {
      limit: RunLimit::Tokens,
    },
  );
  let kept = serde_json::to_string(&end).expect("serialized");
  assert!(kept.contains(r#""ended":{"end":"limit","limit":"tokens"}"#), "{kept}");
  let read: AgentWait = serde_json::from_str(&kept).expect("read back");
  assert_eq!(read, end);
  let question: AgentWait = serde_json::from_str(
    &serde_json::to_string(&AgentWait {
      kind: WaitKind::Question,
      ..end
    })
    .expect("serialized"),
  )
  .expect("read back");
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
    "Merge it only if the person asked you earlier in this conversation",
    "merge_run",
    "Never merge a run nobody asked you to merge",
  ] {
    assert!(ready.contains(part), "{part} in {ready}");
  }
  for (end, says) in [
    (RunEnd::Failed, "failed at the step Review"),
    (RunEnd::Stopped, "stopped at the step Review and waits for a decision"),
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
      "the choices they have",
      "continue_run",
      "allow_retry, or return_to",
      "the person applies",
      "Raising limits or budgets, resuming and accepting a step are the person's",
      "Start, resume, cancel or merge nothing",
    ] {
      assert!(prompt.contains(part), "{end:?}: {part} in {prompt}");
    }
    assert!(!prompt.contains("merge_run"), "{end:?} merges nothing: {prompt}");
  }
}

#[test]
fn orchestrator_system_prompt_proposes_only_a_retry_or_a_return() {
  let prompt = system_prompt("Kontur", ProjectId::generate());
  for rule in [
    "how a stopped run continues",
    "continue_run proposes how a run that waits for a decision goes on: allow_retry",
    "return_to (an earlier agent step, with a note for its agent)",
    "Raising limits or budgets, resuming after the daily budget and accepting a step are the person's",
  ] {
    assert!(prompt.contains(rule), "the system prompt says: {rule}");
  }
}
