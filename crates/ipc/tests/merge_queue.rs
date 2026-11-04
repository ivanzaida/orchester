//! The merge queue's work in a run's worktree (protocol 41, ORC-684): every request and reply crosses a connection as
//! it was sent. `cargo test -p orchester-ipc --test merge_queue merge_queue`.

use orchester_core::{
  id::RunId,
  integration::delivery::Synced,
  merge_queue::recheck::{RecheckTool, Rechecked},
  resource::{CommandAction, CommandProgram},
  revision::Revision,
  run::BranchName,
};
use orchester_ipc::{
  Command, Event, PROTOCOL_VERSION, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter},
  merge_queue::{MergeQueueCommand, MergeQueueReply},
};

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(40)).expect("revision")
}

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("branch")
}

fn requests(run: RunId) -> Vec<Request> {
  let tool = RecheckTool {
    name: "Run tests".to_owned(),
    action: CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new()),
  };
  [
    MergeQueueCommand::SyncBase {
      run,
      base: branch("main"),
      branch: branch("orchester/orc-1-run-1"),
    },
    MergeQueueCommand::Recheck { run, tools: vec![tool] },
    MergeQueueCommand::Recheck { run, tools: Vec::new() },
  ]
  .into_iter()
  .zip(1..)
  .map(|(command, id)| Request {
    id: RequestId::new(id),
    command: Command::MergeQueue(command),
  })
  .collect()
}

fn replies() -> Vec<Event> {
  [
    MergeQueueReply::Synced(Synced::Clean {
      base: revision('a'),
      tip: revision('b'),
    }),
    MergeQueueReply::Synced(Synced::Conflicts {
      base: revision('a'),
      files: vec!["shared.txt".to_owned()],
    }),
    MergeQueueReply::SyncFailed {
      detail: "the base branch main does not exist".to_owned(),
    },
    MergeQueueReply::Rechecked(Rechecked::Passed {
      tools: "Run tests, Lint".to_owned(),
    }),
    MergeQueueReply::Rechecked(Rechecked::NothingToRun),
    MergeQueueReply::Rechecked(Rechecked::Failed {
      tool: "Run tests".to_owned(),
      detail: "1 failed\nexit 1".to_owned(),
    }),
  ]
  .into_iter()
  .zip(1..)
  .map(|(reply, id)| Event::Reply {
    id: RequestId::new(id),
    reply: Reply::MergeQueue(reply),
  })
  .collect()
}

#[tokio::test]
async fn merge_queue_requests_and_replies_cross_a_connection_unchanged() {
  assert_eq!(
    PROTOCOL_VERSION, 43,
    "the merge queue's work came with protocol 41, the executor's folders with 42, the chat's git with 43"
  );
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _) = tokio::io::split(left);
  let (_, write) = tokio::io::split(right);
  let (mut reader, mut writer) = (FrameReader::new(read), FrameWriter::new(write));
  let sent = requests(RunId::generate());
  let events = replies();
  let written = async {
    for request in &sent {
      writer.write(request).await.expect("write request");
    }
    for event in &events {
      writer.write_event(event).await.expect("write event");
    }
    drop(writer);
  };
  let read = async {
    let mut requests = Vec::new();
    for _ in 0..sent.len() {
      requests.push(reader.read::<Request>().await.expect("read").expect("a request"));
    }
    let mut replies = Vec::new();
    while let Some(event) = reader.read_event().await.expect("read event") {
      replies.push(event);
    }
    (requests, replies)
  };
  let ((), (requests, received)) = tokio::join!(written, read);
  assert_eq!(requests, sent);
  assert_eq!(received, events);
}
