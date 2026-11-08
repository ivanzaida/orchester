//! The Orchestrator chat follows the ends of its project's runs and decides how a stopped run goes on
//! (`docs/design/orchestrator-toolset.md`, decision 5, ORC-909): the prompt of an end's turn says what to do with each
//! end, deciding with `continue_run` for a run that waits for a decision or stopped at a limit; and the system prompt
//! names what the chat does while it drives and what it only proposes while it does not.

use orchester_core::{
  id::{ProjectId, RunId},
  orchestrator::{
    AgentWait, Continuing, MergeLeft, Merging, RunEnd, StopReason, WaitKind,
    instructions::{ProjectReads, system_prompt, wait_prompt},
  },
  run::RunLimit,
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
    automatic: false,
    continuing: None,
    merging: None,
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
    "Let the Orchestrator drive is on",
    "Call merge_run for it only if the person asked you earlier in this conversation",
    "never merge a run nobody asked you to merge",
  ] {
    assert!(ready.contains(part), "{part} in {ready}");
  }
  let stopped = wait_prompt(&ended(run, STOPPED));
  for part in [
    "stopped at the step Review (its step used up its 2 retries) and waits for a decision",
    "Decide how it goes on with continue_run",
    "accept the step as it is when its report found no real mismatch",
    "return_to an earlier agent step with a note",
    "Start, resume, cancel or merge nothing",
  ] {
    assert!(stopped.contains(part), "{part} in {stopped}");
  }
  let failed = wait_prompt(&ended(run, RunEnd::Failed));
  for part in ["failed at the step Review", "do it with continue_run (return_to)"] {
    assert!(failed.contains(part), "{part} in {failed}");
  }
  assert!(
    !failed.contains("allow_retry") && !failed.contains("merge_run"),
    "{failed}"
  );
  let limit = wait_prompt(&ended(
    run,
    RunEnd::Limit {
      limit: RunLimit::DailyBudget,
    },
  ));
  for part in [
    "stopped at the project's daily token budget",
    "Read the run first",
    "Decide with continue_run: raise_limits or raise_step_budget",
    "say by how much and why",
    "Start, cancel or merge nothing",
  ] {
    assert!(limit.contains(part), "{part} in {limit}");
  }
  assert!(!limit.contains("merge_run"), "a limit invites no merge: {limit}");
  let interrupted = wait_prompt(&ended(run, RunEnd::Interrupted));
  for part in [
    "was interrupted",
    "Read the run first",
    "the choices they have on the task's Summary",
    "this interruption is not one you continue",
    "Start, resume, cancel, continue or merge nothing",
  ] {
    assert!(interrupted.contains(part), "{part} in {interrupted}");
  }
  assert!(
    !interrupted.contains("allow_retry") && !interrupted.contains("merge_run"),
    "an interruption the chat may not continue invites no retry and no merge: {interrupted}"
  );
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
fn orchestrator_run_end_prompt_of_contention_tells_the_person_and_continues_nothing() {
  let wait = AgentWait {
    automatic: true,
    continuing: Some(Continuing::StorageContention),
    ..ended(RunId::generate(), RunEnd::Interrupted)
  };
  let prompt = wait_prompt(&wait);
  for part in [
    "Run #3 of KON-12 (Sync contacts) was interrupted",
    "the project database stayed busy, and more than 3 runs of the project were stopped so within 10 minutes",
    "do not continue it",
    "the database is under contention",
    "suggest running fewer agents at once",
    "Start, resume, cancel, continue or merge nothing",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
  let once = wait_prompt(&AgentWait {
    continuing: Some(Continuing::Continues),
    ..wait
  });
  assert!(once.contains("Continue this run once with resume_run"), "{once}");
}

#[test]
fn orchestrator_system_prompt_decides_while_driving_and_proposes_otherwise() {
  let prompt = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
  for rule in [
    "decide how a stopped run continues",
    "Who decides: the project's switch Let the Orchestrator drive",
    "you never tell the person to press a button a tool of yours can press",
    "continue_run decides how a run that waits for a decision or stopped at a limit goes on",
    "accept a step as it is when its report found no real mismatch",
    "raise_limits or raise_step_budget",
    "While the switch is off, allow_retry, return_to and resolve_conflicts are proposed to the person",
    "from its last checkpoint, while Let the Orchestrator drive is on",
    "one whose worktree is gone are refused",
    "no turn starts by itself",
  ] {
    assert!(prompt.contains(rule), "the system prompt says: {rule}");
  }
}

#[test]
fn orchestrator_run_ends_of_a_success_merge_it_once_or_say_why_it_is_left_to_the_person() {
  let run = RunId::generate();
  let merging = AgentWait {
    automatic: true,
    merging: Some(Merging::Merges),
    ..ended(run, RunEnd::Succeeded)
  };
  let prompt = wait_prompt(&merging);
  for part in [
    "Let the Orchestrator drive is on",
    "Merge this run once with merge_run (run #3)",
    "call continue_run with resolve_conflicts for this run",
    "do not call merge_run again",
    "needs_decision or scoped_integration_required",
    "leave the merge to them",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
  assert!(
    !prompt.contains("never merge a run nobody asked you to merge"),
    "{prompt}"
  );
  let left = AgentWait {
    merging: Some(Merging::Left {
      why: MergeLeft::BlockedByTasks {
        tasks: vec!["KON-3".to_owned(), "KON-4".to_owned()],
      },
    }),
    ..merging.clone()
  };
  let prompt = wait_prompt(&left);
  for part in [
    "This run is left to the person: unfinished tasks block its task (KON-3, KON-4)",
    "Do not merge it",
    "why it was not merged on its own",
    "Start, resume, cancel, continue or merge nothing",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
  let opted_out = AgentWait {
    merging: Some(Merging::Left {
      why: MergeLeft::OptedOut,
    }),
    ..merging
  };
  assert!(
    wait_prompt(&opted_out).contains("its task says Don't merge automatically"),
    "{}",
    wait_prompt(&opted_out)
  );
  let system = system_prompt("Kontur", ProjectId::generate(), ProjectReads::FileTools);
  assert!(
    system.contains("Orchester's message for a run's success tells you to merge it on your own"),
    "{system}"
  );
}
