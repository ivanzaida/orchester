//! A run's folder state asked of the runner for the start-up recovery and its answers (protocol 43) travel as frames
//! unchanged: every state, and the first file not committed as git named it.

use orchester_core::id::RunId;
use orchester_ipc::{
  Command, Event, FolderState, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter},
};

#[tokio::test]
async fn frames_round_trip_a_run_folder_state_request_and_every_answer() {
  let (near, far) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(far), FrameWriter::new(near));
  let run = RunId::generate();
  let request = Request {
    id: RequestId::new(7),
    command: Command::RunFolderState { run },
  };
  let states = [
    FolderState::Clean,
    FolderState::Uncommitted {
      files: 3,
      first: Some("src/é \"quoted\".rs".to_owned()),
    },
    FolderState::Uncommitted { files: 1, first: None },
    FolderState::Unknown,
  ];
  let replies: Vec<Event> = states
    .into_iter()
    .map(|state| Event::Reply {
      id: RequestId::new(7),
      reply: Reply::RunFolderState { run, state },
    })
    .collect();
  writer.write(&request).await.expect("write the request");
  for reply in &replies {
    writer.write(reply).await.expect("write a reply");
  }
  drop(writer);
  assert_eq!(reader.read::<Request>().await.expect("read the request"), Some(request));
  for reply in replies {
    assert_eq!(reader.read::<Event>().await.expect("read a reply"), Some(reply));
  }
  assert_eq!(reader.read::<Event>().await.expect("a clean end"), None);
}
