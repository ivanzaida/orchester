//! The desktop's requests about the folders a project's runs leave with their executor (protocol 42): sizes, cleaning,
//! discarding and keeping a warm workspace's files, their replies and a refusal, each read back as written.

use orchester_core::{id::RunId, run::BranchName, workspaces::WorkspaceNumber};
use orchester_ipc::{
  Command, Event, PROTOCOL_VERSION, Rejection, Reply, Request, RequestId,
  folders::{FolderSizes, FoldersCommand, FoldersReply},
  frame::{FrameReader, FrameWriter},
};

#[tokio::test]
async fn folders_requests_and_replies_round_trip_under_protocol_42() {
  assert_eq!(
    PROTOCOL_VERSION, 43,
    "an older runner is told apart at the handshake: the folders came with 42, the chat's git in a run's worktree \
     with 43"
  );
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(left), FrameWriter::new(right));
  let (one, two) = (
    WorkspaceNumber::new(1).expect("above zero"),
    WorkspaceNumber::new(2).expect("above zero"),
  );
  let run = RunId::generate();
  let branch = BranchName::new("orchester/rescue/orc-12-run-3").expect("a valid branch name");
  let requests = [
    FoldersCommand::Sizes {
      workspaces: vec![one, two],
    },
    FoldersCommand::Clean { number: one },
    FoldersCommand::Discard { number: two },
    FoldersCommand::Keep { number: two, branch },
  ]
  .into_iter()
  .enumerate()
  .map(|(id, command)| Request {
    id: RequestId::new(u64::try_from(id).expect("small")),
    command: Command::Folders(command),
  })
  .collect::<Vec<_>>();
  let reply = |reply| Event::Reply {
    id: RequestId::new(9),
    reply,
  };
  let events = vec![
    reply(Reply::Folders(FoldersReply::Sizes(FolderSizes {
      workspaces: vec![(one, Some(80_000)), (two, None)],
      run_worktrees: vec![(run, Some(1_000))],
    }))),
    reply(Reply::Folders(FoldersReply::Cleaned { freed: 79_000 })),
    reply(Reply::Folders(FoldersReply::Discarded)),
    reply(Reply::Folders(FoldersReply::Kept)),
    reply(Reply::Rejected(Rejection::Failed {
      detail: "workspace 2: git refused".to_owned(),
    })),
  ];
  for request in &requests {
    writer.write(request).await.expect("write request");
  }
  for event in &events {
    writer.write(event).await.expect("write event");
  }
  drop(writer);
  for request in requests {
    assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
  }
  for event in events {
    assert_eq!(reader.read::<Event>().await.expect("read event"), Some(event));
  }
}
