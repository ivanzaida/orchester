use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  resource::StepKey,
  run::{PreflightProblem, RunLimit},
  tokens::TokenBudget,
};
use orchester_ipc::{
  Command, Event, FrameError, Handshake, IpcError, PROTOCOL_VERSION, Rejection, Reply, Request, RequestId,
  ShutdownMode,
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

#[tokio::test]
async fn frames_round_trip_every_message() {
  let ((mut reader, _), (_, mut writer)) = connection();
  let (run, task) = (RunId::generate(), TaskId::generate());
  let commands = [
    Command::Attach,
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
    Command::Shutdown {
      mode: ShutdownMode::FinishAtCheckpoints,
    },
    Command::Shutdown {
      mode: ShutdownMode::Immediate,
    },
  ]
  .into_iter()
  .enumerate()
  .map(|(index, command)| Request {
    id: RequestId::new(index as u64 + 1),
    command,
  })
  .collect::<Vec<_>>();
  let reply = |reply| Event::Reply {
    id: RequestId::new(7),
    reply,
  };
  let events = [
    reply(Reply::Attached { active_runs: vec![run] }),
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
        PreflightProblem::ApprovalGate {
          step: StepKey::new("plan").expect("key"),
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
  ];

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
