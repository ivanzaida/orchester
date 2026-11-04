//! The files of a run's worktree on the wire (protocol 47): the requests and every reply round-trip, a file's bytes
//! travel raw after the reply in a binary frame, the largest file the viewer shows and a listing's page fit a frame,
//! and a file's reply without its bytes is refused.

use orchester_core::id::RunId;
use orchester_ipc::{
  Command, Event, FrameError, PROTOCOL_VERSION, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter, MAX_FRAME_LEN},
  run_files::{
    FileBytes, IoKind, MAX_LIST_PAGE_LEN, MAX_VIEW_BYTES, RunFileFailure, RunFileRead, RunFilesReply, RunFilesRequest,
  },
};
use tokio::io::{AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf};

type Reader = FrameReader<ReadHalf<DuplexStream>>;
type Writer = FrameWriter<WriteHalf<DuplexStream>>;

/// One end that reads and one that writes, of an in-memory connection.
fn connection() -> (Reader, Writer) {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _) = tokio::io::split(left);
  let (_, write) = tokio::io::split(right);
  (FrameReader::new(read), FrameWriter::new(write))
}

/// Writes `sent` on one end of a connection and reads every event back on the other.
async fn round_trip(sent: &[Event]) -> Vec<Event> {
  let (mut reader, mut writer) = connection();
  let written = async {
    for event in sent {
      writer.write_event(event).await.expect("write event");
    }
    drop(writer);
  };
  let read = async {
    let mut events = Vec::new();
    while let Some(event) = reader.read_event().await.expect("read event") {
      events.push(event);
    }
    events
  };
  let ((), received) = tokio::join!(written, read);
  received
}

fn reply(id: u64, reply: RunFilesReply) -> Event {
  Event::Reply {
    id: RequestId::new(id),
    reply: Reply::RunFiles(reply),
  }
}

#[test]
fn run_files_requests_round_trip_under_a_raised_protocol() {
  assert_eq!(PROTOCOL_VERSION, 47);
  let run = RunId::generate();
  for request in [
    RunFilesRequest::Present,
    RunFilesRequest::List { from: 0 },
    RunFilesRequest::List { from: 120_000 },
    RunFilesRequest::Read {
      path: "src/main.rs".to_owned(),
    },
  ] {
    let sent = Request {
      id: RequestId::new(7),
      command: Command::RunFiles { run, request },
    };
    let json = serde_json::to_string(&sent).expect("JSON");
    assert_eq!(serde_json::from_str::<Request>(&json).expect("read back"), sent);
  }
}

#[tokio::test]
async fn run_files_replies_round_trip_and_a_files_bytes_travel_raw() {
  let mut largest = vec![b'x'; usize::try_from(MAX_VIEW_BYTES).expect("fits")];
  largest[3] = 0;
  let sent = [
    reply(1, RunFilesReply::Gone),
    reply(2, RunFilesReply::Present),
    reply(
      3,
      RunFilesReply::Listed {
        files: vec!["src/main.rs".to_owned(), "notes.md".to_owned()],
        next: None,
      },
    ),
    reply(
      10,
      RunFilesReply::Listed {
        files: vec!["a.rs".to_owned()],
        next: Some(1),
      },
    ),
    reply(4, RunFilesReply::Read(RunFileRead::Bytes(FileBytes { bytes: largest }))),
    reply(5, RunFilesReply::Read(RunFileRead::Bytes(FileBytes::default()))),
    reply(
      6,
      RunFilesReply::Read(RunFileRead::TooLarge {
        size: MAX_VIEW_BYTES + 1,
      }),
    ),
    reply(7, RunFilesReply::Failed(RunFileFailure::Missing)),
    reply(
      8,
      RunFilesReply::Failed(RunFileFailure::Unreadable {
        kind: IoKind::PermissionDenied,
      }),
    ),
    reply(
      9,
      RunFilesReply::Failed(RunFileFailure::Git {
        detail: "fatal".to_owned(),
      }),
    ),
  ];
  assert!(
    MAX_VIEW_BYTES < u64::from(MAX_FRAME_LEN),
    "the largest file fits a frame"
  );
  assert!(
    MAX_LIST_PAGE_LEN * 2 < MAX_FRAME_LEN as usize,
    "a listing's page fits a frame with room to spare"
  );
  assert_eq!(round_trip(&sent).await, sent);
}

#[test]
fn run_files_io_kinds_keep_their_name_and_others_are_other() {
  for kind in [
    std::io::ErrorKind::NotFound,
    std::io::ErrorKind::PermissionDenied,
    std::io::ErrorKind::Unsupported,
  ] {
    assert_eq!(std::io::ErrorKind::from(IoKind::from(kind)), kind);
  }
  assert_eq!(IoKind::from(std::io::ErrorKind::AddrInUse), IoKind::Other);
}

#[tokio::test]
async fn run_files_a_files_reply_without_its_bytes_is_refused() {
  let event = reply(1, RunFilesReply::Read(RunFileRead::Bytes(FileBytes::default())));
  let head = serde_json::to_vec(&event).expect("JSON");
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _keep) = tokio::io::split(left);
  let mut reader = FrameReader::new(read);
  let (_, mut raw) = tokio::io::split(right);
  let len = u32::try_from(head.len()).expect("small");
  raw.write_all(&len.to_be_bytes()).await.expect("write header");
  raw.write_all(&head).await.expect("write body");
  drop(raw);
  let read = reader.read_event().await;
  assert!(matches!(read, Err(FrameError::Malformed(_))), "{read:?}");
}
