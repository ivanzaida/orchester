use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, ResourceId, RunId, StepExecutionId, TaskId},
  integration::{FolderListing, LeftWorktree, Leftovers, WorktreeOwner},
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
  Request, RequestId, RunWork, SessionEnd, ShutdownMode, SlotWait, WorkState, WorkSummary,
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
    Command::SetAgentSlots {
      agent_slots: NonZeroU32::new(16).expect("above zero"),
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
      budget: Some(TokenBudget::new(2_000_000).expect("above zero")),
      max_duration_ms: Some(1_800_000),
    },
    Command::RaiseLimit {
      run,
      budget: None,
      max_duration_ms: None,
    },
    Command::RaiseStepBudget {
      run,
      budget: Some(TokenBudget::new(600_000).expect("above zero")),
    },
    Command::RaiseStepBudget { run, budget: None },
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
  .chain(decision_commands(run))
  .chain([Command::DeliverMessages { run }])
  .enumerate()
  .map(|(index, command)| Request {
    id: RequestId::new(index as u64 + 1),
    command,
  })
  .collect()
}

/// Decisions on plans: automation's approval, and a person's request for changes with a note (protocol 20);
/// and an answer to a question with suggested answers.
fn decision_commands(run: RunId) -> Vec<Command> {
  let note = ReviewNote::new("Keep email sign-in.\n\"Quoted\" — é").expect("a note");
  vec![
    Command::DecideStuckRunAs {
      run,
      expected: orchester_core::run::EventSequence::FIRST,
      decision: orchester_core::run::StuckDecision::Retry,
      by: orchester_core::run::Answerer::Automation,
    },
    Command::AnswerQuestion {
      run,
      request: "invocation:7".to_owned(),
      answers: vec![orchester_core::question::GivenAnswer {
        choices: vec!["Under Sign in".to_owned()],
        text: Some("and the footer — é".to_owned()),
      }],
      by: orchester_core::run::Answerer::Automation,
    },
    Command::DecidePlan {
      run,
      execution: StepExecutionId::generate(),
      version: PlanVersion::FIRST,
      decision: PlanDecision::Approved,
      by: orchester_core::run::Answerer::Automation,
    },
    Command::DecidePlan {
      run,
      execution: StepExecutionId::generate(),
      version: PlanVersion::FIRST.next(),
      decision: PlanDecision::ChangesRequested { note },
      by: orchester_core::run::Answerer::Person,
    },
  ]
}

/// A plan proposed, a decision on a plan that no longer waits, and an answer to a tool request answered
/// already.
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
    Event::Reply {
      id: RequestId::new(10),
      reply: Reply::Rejected(Rejection::ToolApprovalAnswered),
    },
  ]
}

/// A question an agent asked, messages to a run that ended, and answers to questions with suggested
/// answers refused.
fn conversation_events(run: RunId, task: TaskId) -> Vec<Event> {
  vec![
    Event::QuestionAsked {
      run,
      task,
      task_key: "ORC-12".to_owned(),
      agent: "Planner".to_owned(),
    },
    Event::RunReady {
      run,
      task,
      number: orchester_core::run::RunNumber::new(std::num::NonZeroU32::MIN),
    },
    Event::Reply {
      id: RequestId::new(10),
      reply: Reply::Rejected(Rejection::RunEnded),
    },
    Event::QuestionChanged { run, task },
    Event::Reply {
      id: RequestId::new(11),
      reply: Reply::Rejected(Rejection::InvalidAnswer {
        error: orchester_core::question::AnswerError::UnknownChoice {
          label: "Nowhere".to_owned(),
        },
      }),
    },
    Event::Reply {
      id: RequestId::new(12),
      reply: Reply::Rejected(Rejection::QuestionAnswered),
    },
    Event::Reply {
      id: RequestId::new(13),
      reply: Reply::Rejected(Rejection::QuestionNotAwaited),
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
    reply(Reply::AgentSlotsSet {
      agent_slots: NonZeroU32::new(16).expect("above zero"),
    }),
    reply(Reply::RunStarted { run }),
    reply(Reply::Accepted { run }),
    reply(Reply::Rejected(Rejection::RunNotFound)),
    reply(Reply::Rejected(Rejection::LimitNotRaised { limit: RunLimit::Time })),
    reply(Reply::Rejected(Rejection::LimitNotAboveUsed {
      limit: RunLimit::Tokens,
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
      slot_waits: vec![SlotWait {
        run,
        execution: StepExecutionId::generate(),
      }],
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

/// *Test connection* of a native connection (protocol 17): the model API asked about, and what the runner
/// found with the models it listed.
#[tokio::test]
async fn frames_round_trip_a_connection_check() {
  use orchester_core::{
    connection::{ApiEndpoint, BaseUrl, CheckResult, Effort, EndpointPreset, ModelSource, NativeModel},
    id::SecretId,
    resource::ModelId,
    time::Timestamp,
  };
  let ((mut reader, _), (_, mut writer)) = connection();
  let api = ApiEndpoint::new(
    EndpointPreset::Official,
    BaseUrl::new("https://api.anthropic.com").expect("base URL"),
    SecretId::generate(),
  );
  let request = Request {
    id: RequestId::new(3),
    command: Command::CheckConnection {
      provider: AgentProvider::AnthropicApi,
      api,
    },
  };
  let model = NativeModel::new(
    ModelId::new("claude-opus-5-5").expect("model"),
    NonZeroU32::new(1_000_000),
    NonZeroU32::new(128_000),
    vec![Effort::Low, Effort::Max],
    ModelSource::Endpoint {
      at: Timestamp::from_unix_millis(1_000),
    },
  );
  let reply = Event::Reply {
    id: RequestId::new(3),
    reply: Reply::ConnectionChecked {
      result: CheckResult::Api { balance: None },
      models: vec![model],
    },
  };
  writer.write(&request).await.expect("write request");
  writer.write(&reply).await.expect("write reply");
  drop(writer);
  assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
  assert_eq!(reader.read::<Event>().await.expect("read reply"), Some(reply));
}

/// Run again with what a person saw the run before leave, a worktree folder git forgot among it
/// (protocol 15), listed only in part (protocol 16); a peer that sends no `forgotten` sends a
/// worktree git lists, with all of its files.
#[tokio::test]
async fn frames_round_trip_a_forgotten_worktree() {
  let ((mut reader, _), (_, mut writer)) = connection();
  let worktree = LeftWorktree {
    path: "C:/repo/.orchester/worktrees/01J".to_owned(),
    owner: WorktreeOwner::Run,
    branch: None,
    uncommitted: vec!["target/held.bin".to_owned()],
    missing: false,
    forgotten: true,
    listing: FolderListing::Cut,
  };
  let request = Request {
    id: RequestId::new(1),
    command: Command::RunAgain {
      key: IdempotencyKey::generate(),
      task: TaskId::generate(),
      after: RunId::generate(),
      remove_worktrees: Some(Leftovers {
        worktrees: vec![worktree.clone()],
        ..Leftovers::default()
      }),
      revision: None,
    },
  };
  writer.write(&request).await.expect("write request");
  drop(writer);
  assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));

  let mut older = serde_json::to_value(&worktree).expect("to JSON");
  let fields = older.as_object_mut().expect("an object");
  fields.remove("forgotten");
  fields.remove("listing");
  let listed: LeftWorktree = serde_json::from_value(older).expect("from JSON");
  assert!(!listed.forgotten);
  assert_eq!(listed.listing, FolderListing::Whole);
}

/// A forgotten folder whose listing stopped at an entry that could not be read travels with which
/// entry and why (protocol 16), so the runner removes it only as the person saw it.
#[tokio::test]
async fn frames_round_trip_an_unreadable_forgotten_worktree() {
  let ((mut reader, _), (_, mut writer)) = connection();
  let worktree = LeftWorktree {
    path: "C:/repo/.orchester/worktrees/01J".to_owned(),
    owner: WorktreeOwner::Run,
    branch: None,
    uncommitted: vec!["a.txt".to_owned()],
    missing: false,
    forgotten: true,
    listing: FolderListing::Unreadable {
      path: "C:/repo/.orchester/worktrees/01J/locked".to_owned(),
      detail: "Access is denied. (os error 5)".to_owned(),
    },
  };
  let request = Request {
    id: RequestId::new(1),
    command: Command::RunAgain {
      key: IdempotencyKey::generate(),
      task: TaskId::generate(),
      after: RunId::generate(),
      remove_worktrees: Some(Leftovers {
        worktrees: vec![worktree],
        ..Leftovers::default()
      }),
      revision: None,
    },
  };
  writer.write(&request).await.expect("write request");
  drop(writer);
  assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
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
