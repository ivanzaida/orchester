//! The runner of the open project as the desktop sees it: where the program is found, the
//! state machine with its reconnect policy, and how closing decides the session end.

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_desktop::{
  app_paths::runner_beside,
  close::{CloseKind, ClosePlan, close_plan, session_end},
  runner::{
    ActiveWork, ShutdownOutcome,
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
    RunnerState::Online { reconnected: false }
  );
}

#[test]
fn runner_state_that_cannot_start_fails_without_retrying() {
  assert_eq!(
    apply(
      RunnerState::Starting,
      [RunnerEvent::LaunchFailed(RunnerFailure::StartFailed)]
    ),
    RunnerState::Failed(RunnerFailure::StartFailed)
  );
}

#[test]
fn runner_state_drop_reconnects_then_reports_reconnected() {
  let online = RunnerState::Online { reconnected: false };
  let dropped = online.apply(RunnerEvent::Dropped(RunnerFailure::Lost));
  assert_eq!(
    dropped,
    RunnerState::Reconnecting {
      attempt: 1,
      cause: RunnerFailure::Lost
    }
  );
  let back = dropped.apply(RunnerEvent::Launched);
  assert_eq!(back, RunnerState::Online { reconnected: true });
  assert_eq!(
    back.apply(RunnerEvent::ReconnectNoticeExpired),
    RunnerState::Online { reconnected: false }
  );
}

#[test]
fn runner_state_reconnect_delays_back_off_up_to_eight_seconds() {
  let delays: Vec<Duration> = (1..=RECONNECT_LIMIT).map(reconnect_delay).collect();
  let expected = [500, 1000, 2000, 4000, 8000, 8000].map(Duration::from_millis);
  assert_eq!(delays, expected);

  let mut state = RunnerState::Online { reconnected: false }.apply(RunnerEvent::Dropped(RunnerFailure::Lost));
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
    RunnerState::Online { reconnected: false },
    std::iter::once(RunnerEvent::Dropped(RunnerFailure::Lost)).chain(failed_attempts),
  );
  assert_eq!(state, RunnerState::Failed(RunnerFailure::StartFailed));
  assert!(state.can_restart());
  assert_eq!(state.reconnect_delay(), None);
}

#[test]
fn runner_state_does_not_retry_a_missing_program() {
  let missing = RunnerFailure::MissingExecutable {
    path: PathBuf::from("orchester-runner.exe"),
  };
  let state = apply(
    RunnerState::Online { reconnected: false },
    [
      RunnerEvent::Dropped(RunnerFailure::Lost),
      RunnerEvent::LaunchFailed(missing.clone()),
    ],
  );
  assert_eq!(state, RunnerState::Failed(missing));
}

#[test]
fn runner_state_manual_restart_resets_the_attempts() {
  let failed = RunnerState::Failed(RunnerFailure::Lost);
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
  let online = RunnerState::Online { reconnected: false };
  assert!(!online.can_restart());
  assert_eq!(online.clone().apply(RunnerEvent::RestartRequested), online);
}

#[test]
fn runner_state_stops_when_asked() {
  for running in [
    RunnerState::Starting,
    RunnerState::Online { reconnected: false },
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
      RunnerState::Online { reconnected: false },
      [RunnerEvent::StopRequested, RunnerEvent::Dropped(RunnerFailure::Lost)]
    ),
    RunnerState::Failed(RunnerFailure::Lost)
  );
}

#[test]
fn close_without_active_work_stops_quietly() {
  assert_eq!(close_plan(Vec::new()), ClosePlan::StopQuietly);
}

#[test]
fn close_with_active_work_asks_first() {
  let work = ActiveWork::new("ORC-1 Fix login", "Implement", "Codex");
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
