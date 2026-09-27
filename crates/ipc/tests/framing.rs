use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, ResourceId, RunId, StepExecutionId, TaskId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, PermissionProblem, Unavailable},
  },
  resource::StepKey,
  run::{PermissionsApply, PlanDecision, PlanVersion, PreflightProblem, ReviewNote, RunLimit},
  tokens::TokenBudget,
};
use orchester_ipc::{
  AttachSession, Command, Event, FrameError, Handshake, IpcError, PROTOCOL_VERSION, RefusedRun, Rejection, Reply,
  Request, RequestId, RunWork, SessionEnd, ShutdownMode, WorkState, WorkSummary,
  frame::{FrameReader, FrameWriter, MAX_FRAME_LEN},
  handshake,
};
use tokio::io::{AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf};

type Reader = FrameReader<ReadHalf<DuplexStream>>;
type Writer = FrameWriter<WriteHalf<DuplexStream>>;

/// Both ends of an in-memory connection.
fn connection() -> ((Reader, Writer), (Reader, Writer)) {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let framed = |stream| {
    let (read, write) = tokio::io::split(stream);
    (FrameReader::new(read), FrameWriter::new(write))
  };
  (framed(left), framed(right))
}

/// One request of every command.
fn requests(run: RunId, task: TaskId) -> Vec<Request> {
  [
    Command::Attach {
      session: AttachSession::Fresh {
        previous_end: SessionEnd::QuitNow,
      },
      agent_slots: NonZeroU32::new(4).expect("above zero"),
    },
    Command::Attach {
      session: AttachSession::Reconnect,
      agent_slots: NonZeroU32::new(1).expect("above zero"),
    },
    Command::ContinueRuns { runs: vec![run] },
    Command::Ping { nonce: 42 },
    Command::StartRun {
      key: IdempotencyKey::generate(),
      task,
    },
    Command::PauseRun { run },
    Command::ResumeRun { run },
    Command::CancelRun { run },
    Command::RaiseLimit {
      run,
      budget: TokenBudget::new(2_000_000).expect("above zero"),
      max_duration_ms: 1_800_000,
    },
    Command::RaiseStepBudget {
      run,
      budget: TokenBudget::new(600_000).expect("above zero"),
    },
    Command::ChangeRunPermissions {
      run,
      profile: ResourceId::generate(),
      permissions: AgentPermissions::new(
        RepositoryAccess::Write,
        CommandPermission::Allowed(
          AllowedCommands::new(vec![
            CommandPrefix::new("pnpm test").expect("command"),
            CommandPrefix::new("pnpm build").expect("command"),
          ])
          .expect("list"),
        ),
        NetworkAccess::Off,
      ),
      apply: PermissionsApply::Now,
    },
    Command::ChangeRunPermissions {
      run,
      profile: ResourceId::generate(),
      permissions: AgentPermissions::before_permissions(),
      apply: PermissionsApply::NextInvocation,
    },
    Command::KeepPermissionsForNextInvocation {
      run,
      profile: ResourceId::generate(),
    },
    Command::Shutdown {
      mode: ShutdownMode::FinishAtCheckpoints,
    },
    Command::Shutdown {
      mode: ShutdownMode::Immediate,
    },
  ]
  .into_iter()
  .chain(plan_commands(run))
  .chain([Command::DeliverMessages { run }])
  .enumerate()
  .map(|(index, command)| Request {
    id: RequestId::new(index as u64 + 1),
    command,
  })
  .collect()
}

/// Decisions on plans: an approval, and a request for changes with a note.
fn plan_commands(run: RunId) -> Vec<Command> {
  let note = ReviewNote::new("Keep email sign-in.\n\"Quoted\" — é").expect("a note");
  vec![
    Command::DecidePlan {
      run,
      execution: StepExecutionId::generate(),
      version: PlanVersion::FIRST,
      decision: PlanDecision::Approved,
    },
    Command::DecidePlan {
      run,
      execution: StepExecutionId::generate(),
      version: PlanVersion::FIRST.next(),
      decision: PlanDecision::ChangesRequested { note },
    },
  ]
}

/// A plan proposed, and a decision on a plan that no longer waits.
fn plan_events(run: RunId, task: TaskId) -> Vec<Event> {
  vec![
    Event::PlanProposed {
      run,
      task,
      task_key: "ORC-12".to_owned(),
      version: PlanVersion::FIRST.next(),
    },
    Event::Reply {
      id: RequestId::new(9),
      reply: Reply::Rejected(Rejection::PlanNotAwaited),
    },
  ]
}

/// A question an agent asked, and messages to a run that ended.
fn conversation_events(run: RunId, task: TaskId) -> Vec<Event> {
  vec![
    Event::QuestionAsked {
      run,
      task,
      task_key: "ORC-12".to_owned(),
      agent: "Planner".to_owned(),
    },
    Event::Reply {
      id: RequestId::new(10),
      reply: Reply::Rejected(Rejection::RunEnded),
    },
  ]
}

/// The rejections of a run permissions change.
fn permission_rejections() -> Vec<Rejection> {
  vec![
    Rejection::PermissionsNotEnforceable {
      step: StepKey::new("implement").expect("key"),
      problem: PermissionProblem::NotEnforceable {
        option: PermissionOption::AllowedCommands,
        reason: Unavailable::CannotLimitCommands,
      },
    },
    Rejection::NoAgentToRestart,
    Rejection::NoPendingPermissions,
    Rejection::ProfileNotInRun,
  ]
}

/// One event of every kind, and replies of every kind.
fn events(run: RunId, task: TaskId) -> Vec<Event> {
  let reply = |reply| Event::Reply {
    id: RequestId::new(7),
    reply,
  };
  vec![
    reply(Reply::Attached {
      active_runs: vec![run],
      interrupted: vec![run],
      continued: Vec::new(),
    }),
    reply(Reply::Continued {
      continued: vec![run],
      refused: vec![RefusedRun {
        run,
        rejection: Rejection::DailyBudgetUsed {
          budget: TokenBudget::new(20_000_000).expect("above zero"),
        },
      }],
    }),
    reply(Reply::Rejected(Rejection::NoAgentSlot {
      slots: NonZeroU32::new(4).expect("above zero"),
    })),
    reply(Reply::Rejected(Rejection::RunNotInterrupted)),
    reply(Reply::Rejected(Rejection::ShuttingDown)),
    reply(Reply::Pong { nonce: 42 }),
    reply(Reply::RunStarted { run }),
    reply(Reply::Accepted { run }),
    reply(Reply::Rejected(Rejection::RunNotFound)),
    reply(Reply::Rejected(Rejection::LimitNotRaised { limit: RunLimit::Time })),
    reply(Reply::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::Tokens,
    })),
    reply(Reply::Rejected(Rejection::BudgetAboveMaximum {
      max: TokenBudget::new(10_000_000).expect("above zero"),
    })),
    reply(Reply::Rejected(Rejection::Preflight {
      problems: vec![
        PreflightProblem::ToolAccessRules {
          step: StepKey::new("deploy").expect("key"),
        },
        PreflightProblem::NoCommits,
      ],
    })),
    reply(Reply::Rejected(Rejection::Failed {
      detail: "héllo \"quoted\"\n inner".to_owned(),
    })),
    Event::Stopped {
      mode: ShutdownMode::Immediate,
    },
    Event::RunChanged { run, task },
    Event::AgentActivity { run, sequence: 12 },
    Event::WorkSummary(WorkSummary {
      agent_slots: NonZeroU32::new(4).expect("above zero"),
      runs: [
        WorkState::Working,
        WorkState::Saving,
        WorkState::Saved {
          checkpoint: Some("9d07b3f".repeat(5) + "12345"),
        },
        WorkState::Saved { checkpoint: None },
      ]
      .into_iter()
      .map(|state| RunWork {
        run,
        task,
        task_key: "ORC-125".to_owned(),
        task_title: "Fix canvas zoom".to_owned(),
        step: "Implement".to_owned(),
        agent: "Builder".to_owned(),
        provider: AgentProvider::CodexCli,
        model: "gpt-5.5".to_owned(),
        state,
      })
      .collect(),
    }),
  ]
}

#[tokio::test]
async fn frames_round_trip_every_message() {
  let ((mut reader, _), (_, mut writer)) = connection();
  let (run, task) = (RunId::generate(), TaskId::generate());
  let commands = requests(run, task);
  let rejected = permission_rejections().into_iter().map(|rejection| Event::Reply {
    id: RequestId::new(8),
    reply: Reply::Rejected(rejection),
  });
  let events: Vec<Event> = events(run, task)
    .into_iter()
    .chain(plan_events(run, task))
    .chain(conversation_events(run, task))
    .chain(rejected)
    .collect();
  for request in &commands {
    writer.write(request).await.expect("write request");
  }
  for event in &events {
    writer.write(event).await.expect("write event");
  }
  drop(writer);

  for request in commands {
    assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
  }
  for event in events {
    assert_eq!(reader.read::<Event>().await.expect("read event"), Some(event));
  }
  assert_eq!(reader.read::<Event>().await.expect("clean end"), None);
}

#[tokio::test]
async fn frames_reject_oversized_and_truncated_input() {
  let (left, right) = tokio::io::duplex(64);
  let (read, _keep) = tokio::io::split(left);
  let mut reader = FrameReader::new(read);
  let (_, mut raw) = tokio::io::split(right);
  raw
    .write_all(&(MAX_FRAME_LEN + 1).to_be_bytes())
    .await
    .expect("write header");
  assert!(matches!(
    reader.read::<Request>().await,
    Err(FrameError::TooLarge { .. })
  ));

  let (left, right) = tokio::io::duplex(64);
  let (read, _keep) = tokio::io::split(left);
  let mut reader = FrameReader::new(read);
  let (_, mut raw) = tokio::io::split(right);
  raw.write_all(&10_u32.to_be_bytes()).await.expect("write header");
  raw.write_all(b"{\"ty").await.expect("write partial body");
  drop(raw);
  assert!(matches!(reader.read::<Request>().await, Err(FrameError::Truncated)));
}

#[tokio::test]
async fn handshake_succeeds_between_equal_versions() {
  let ((mut left_reader, mut left_writer), (mut right_reader, mut right_writer)) = connection();
  let (left, right) = tokio::join!(
    handshake(&mut left_reader, &mut left_writer),
    handshake(&mut right_reader, &mut right_writer),
  );
  left.expect("left handshake");
  right.expect("right handshake");
}

#[tokio::test]
async fn handshake_reports_a_version_mismatch() {
  let ((mut reader, mut writer), (mut peer_reader, mut peer_writer)) = connection();
  let peer_version = PROTOCOL_VERSION + 1;
  let peer = async {
    peer_writer
      .write(&Handshake::new(peer_version))
      .await
      .expect("peer writes its version");
    peer_reader.read::<Handshake>().await.expect("peer reads the version")
  };
  let (result, received) = tokio::join!(handshake(&mut reader, &mut writer), peer);

  assert_eq!(received, Some(Handshake::new(PROTOCOL_VERSION)));
  match result {
    Err(IpcError::VersionMismatch { local, remote }) => {
      assert_eq!(local, PROTOCOL_VERSION);
      assert_eq!(remote, peer_version);
    }
    other => panic!("expected a version mismatch, got {other:?}"),
  }
}

#[tokio::test]
async fn handshake_reports_a_peer_that_closes() {
  let ((mut reader, mut writer), peer) = connection();
  drop(peer);
  assert!(matches!(
    handshake(&mut reader, &mut writer).await,
    Err(IpcError::ClosedDuringHandshake | IpcError::Frame(FrameError::Io(_)))
  ));
}
