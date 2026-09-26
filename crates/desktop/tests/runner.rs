//! The runner of the open project as the desktop sees it: where the program is found, the
//! state machine with its reconnect policy, and how closing decides the session end.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{connection::AgentProvider, id::RunId};
use orchester_desktop::{
  app_paths::runner_beside,
  close::{CloseKind, ClosePlan, close_plan, session_end},
  messages::{Message, runner_not_restarted},
  runner::{
    ActiveWork, ActiveWorkFields, ShutdownOutcome,
    state::{RECONNECT_LIMIT, RunnerEvent, RunnerFailure, RunnerState, reconnect_delay},
  },
};
use orchester_store::SessionEnd;

#[test]
fn runner_lives_next_to_the_executable() {
  let executable = Path::new("C:/Program Files/Orchester/orchester.exe");
  let expected =
    PathBuf::from("C:/Program Files/Orchester/orchester-runner").with_extension(std::env::consts::EXE_EXTENSION);
  assert_eq!(runner_beside(executable), expected);
}

fn apply(state: RunnerState, events: impl IntoIterator<Item = RunnerEvent>) -> RunnerState {
  events.into_iter().fold(state, RunnerState::apply)
}

#[test]
fn runner_state_starts_then_goes_online() {
  assert_eq!(
    apply(RunnerState::Starting, [RunnerEvent::Launched]),
    RunnerState::Online
  );
}

#[test]
fn runner_state_that_cannot_start_fails_without_retrying() {
  assert_eq!(
    apply(
      RunnerState::Starting,
      [RunnerEvent::LaunchFailed(RunnerFailure::StartFailed)]
    ),
    RunnerState::Failed {
      cause: RunnerFailure::StartFailed,
      attempts: 0
    }
  );
}

#[test]
fn runner_state_drop_reconnects_then_goes_online() {
  let online = RunnerState::Online;
  let dropped = online.apply(RunnerEvent::Dropped(RunnerFailure::Lost));
  assert_eq!(
    dropped,
    RunnerState::Reconnecting {
      attempt: 1,
      cause: RunnerFailure::Lost
    }
  );
  assert_eq!(dropped.apply(RunnerEvent::Launched), RunnerState::Online);
}

#[test]
fn runner_state_reconnect_delays_back_off_up_to_eight_seconds() {
  let delays: Vec<Duration> = (1..=RECONNECT_LIMIT).map(reconnect_delay).collect();
  let expected = [500, 1000, 2000, 4000, 8000].map(Duration::from_millis);
  assert_eq!(delays, expected);

  let mut state = RunnerState::Online.apply(RunnerEvent::Dropped(RunnerFailure::Lost));
  let mut seen = Vec::new();
  while let Some(delay) = state.reconnect_delay() {
    seen.push(delay);
    state = state.apply(RunnerEvent::LaunchFailed(RunnerFailure::StartFailed));
  }
  assert_eq!(seen, expected);
}

#[test]
fn runner_state_gives_up_after_the_reconnect_limit() {
  let failed_attempts = (0..RECONNECT_LIMIT).map(|_| RunnerEvent::LaunchFailed(RunnerFailure::StartFailed));
  let state = apply(
    RunnerState::Online,
    std::iter::once(RunnerEvent::Dropped(RunnerFailure::Lost)).chain(failed_attempts),
  );
  assert_eq!(
    state,
    RunnerState::Failed {
      cause: RunnerFailure::StartFailed,
      attempts: RECONNECT_LIMIT
    }
  );
  assert!(state.can_restart());
  assert_eq!(state.reconnect_delay(), None);
}

#[test]
fn runner_state_gives_up_after_five_attempts_as_designed() {
  // "attempt 2 of 5" (`keJ9Z` E) and "Tried 5 times since 14:02" (02.H `EFY7I`).
  let dropped = RunnerState::Online.apply(RunnerEvent::Dropped(RunnerFailure::Lost));
  let failed = (0..5).fold(dropped, |state, _| {
    assert!(matches!(state, RunnerState::Reconnecting { .. }));
    state.apply(RunnerEvent::LaunchFailed(RunnerFailure::StartFailed))
  });
  assert_eq!(
    failed,
    RunnerState::Failed {
      cause: RunnerFailure::StartFailed,
      attempts: 5
    }
  );
}

#[test]
fn runner_not_restarted_names_the_attempts_and_since_when() {
  // "Tried 5 times since 14:02." (02.H `EFY7I`)
  assert_eq!(
    runner_not_restarted(5, Some("14:02")),
    Message::new("runner.connection.notRestartedSince.other")
      .with("attempts", "5".to_owned())
      .with("time", "14:02".to_owned())
  );
  assert_eq!(
    runner_not_restarted(1, Some("09:30")).key,
    "runner.connection.notRestartedSince.one"
  );
  assert_eq!(
    runner_not_restarted(5, None),
    Message::new("runner.connection.notRestarted.other").with("attempts", "5".to_owned())
  );
}

#[test]
fn runner_state_does_not_retry_a_missing_program() {
  let missing = RunnerFailure::MissingExecutable {
    path: PathBuf::from("orchester-runner.exe"),
  };
  let state = apply(
    RunnerState::Online,
    [
      RunnerEvent::Dropped(RunnerFailure::Lost),
      RunnerEvent::LaunchFailed(missing.clone()),
    ],
  );
  assert_eq!(
    state,
    RunnerState::Failed {
      cause: missing,
      attempts: 1
    }
  );
}

#[test]
fn runner_state_manual_restart_resets_the_attempts() {
  let failed = RunnerState::Failed {
    cause: RunnerFailure::Lost,
    attempts: RECONNECT_LIMIT,
  };
  let restarted = failed.apply(RunnerEvent::RestartRequested);
  assert_eq!(restarted, RunnerState::Starting);
  let dropped_again = apply(
    restarted,
    [RunnerEvent::Launched, RunnerEvent::Dropped(RunnerFailure::Lost)],
  );
  assert_eq!(
    dropped_again,
    RunnerState::Reconnecting {
      attempt: 1,
      cause: RunnerFailure::Lost
    }
  );
}

#[test]
fn runner_state_restart_is_ignored_while_running() {
  let online = RunnerState::Online;
  assert!(!online.can_restart());
  assert_eq!(online.clone().apply(RunnerEvent::RestartRequested), online);
}

#[test]
fn runner_state_stops_when_asked() {
  for running in [
    RunnerState::Starting,
    RunnerState::Online,
    RunnerState::Reconnecting {
      attempt: 2,
      cause: RunnerFailure::Lost,
    },
  ] {
    assert_eq!(
      apply(running, [RunnerEvent::StopRequested, RunnerEvent::Stopped]),
      RunnerState::Stopped
    );
  }
  assert_eq!(
    apply(
      RunnerState::Online,
      [RunnerEvent::StopRequested, RunnerEvent::Dropped(RunnerFailure::Lost)]
    ),
    RunnerState::Failed {
      cause: RunnerFailure::Lost,
      attempts: 0
    }
  );
}

#[test]
fn close_without_active_work_stops_quietly() {
  assert_eq!(close_plan(Vec::new()), ClosePlan::StopQuietly);
}

#[test]
fn close_with_active_work_asks_first() {
  let work = ActiveWork::new(&ActiveWorkFields {
    project: "Kontur",
    run: RunId::generate(),
    task_key: "ORC-1",
    task_title: "Fix login",
    step: "Implement",
    agent: "Builder",
    provider: AgentProvider::CodexCli,
    model: "gpt-5.5",
  });
  assert_eq!(close_plan(vec![work.clone()]), ClosePlan::Confirm(vec![work].into()));
}

#[test]
fn close_session_end_follows_the_choice_and_the_runner_answer() {
  use CloseKind::{Quiet, QuitNow, WaitForAgents};
  use ShutdownOutcome::{NoRunner, Stopped, Unanswered};
  let cases = [
    (Quiet, Stopped, SessionEnd::Clean),
    (Quiet, NoRunner, SessionEnd::Clean),
    (Quiet, Unanswered, SessionEnd::QuitNow),
    (WaitForAgents, Stopped, SessionEnd::RunsPaused),
    (WaitForAgents, NoRunner, SessionEnd::Clean),
    (WaitForAgents, Unanswered, SessionEnd::QuitNow),
    (QuitNow, Unanswered, SessionEnd::QuitNow),
    (QuitNow, Stopped, SessionEnd::QuitNow),
  ];
  for (kind, outcome, end) in cases {
    assert_eq!(session_end(kind, outcome), end, "{kind:?} with {outcome:?}");
  }
}
