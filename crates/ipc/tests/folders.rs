//! The desktop's requests about the folders a project's runs leave with their executor (protocol 42): sizes, cleaning,
//! discarding and keeping a warm workspace's files, their replies and a refusal, each read back as written; and
//! listing them unsized and measuring one (protocol 51).

use orchester_core::{id::RunId, run::BranchName, workspaces::WorkspaceNumber};
use orchester_ipc::{
  Command, Event, PROTOCOL_VERSION, Rejection, Reply, Request, RequestId,
  folders::{ExecutorFolder, FolderList, FolderSizes, FoldersCommand, FoldersReply},
  frame::{FrameReader, FrameWriter},
};

#[tokio::test]
async fn folders_requests_and_replies_round_trip_under_protocol_42() {
  const {
    assert!(
      PROTOCOL_VERSION >= 42,
      "a runner older than the folders requests is told apart at the handshake"
    )
  };
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
      worktrees: vec![(run.to_string(), Some(1_000))],
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

#[tokio::test]
async fn folders_listing_and_measuring_one_folder_round_trip_under_protocol_51() {
  const {
    assert!(
      PROTOCOL_VERSION >= 51,
      "a runner older than listing and measuring one folder is told apart at the handshake"
    )
  };
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(left), FrameWriter::new(right));
  let one = WorkspaceNumber::new(1).expect("above zero");
  let run = RunId::generate();
  let requests = [
    FoldersCommand::List { workspaces: vec![one] },
    FoldersCommand::Measure {
      folder: ExecutorFolder::Worktree { name: run.to_string() },
    },
    FoldersCommand::Measure {
      folder: ExecutorFolder::Workspace { number: one },
    },
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
    reply(Reply::Folders(FoldersReply::Listed(FolderList {
      workspaces: vec![one],
      worktrees: vec![run.to_string()],
    }))),
    reply(Reply::Folders(FoldersReply::Measured { bytes: Some(1_000) })),
    reply(Reply::Folders(FoldersReply::Measured { bytes: None })),
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
