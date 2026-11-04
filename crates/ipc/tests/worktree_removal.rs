//! A removal of a run's worktrees asked of the runner and its answers (protocol 45) travel as frames unchanged: each
//! kind of request, what a removal did with every reason a worktree or branch stays, and every reason it removed
//! nothing.

use orchester_core::{
  id::{RunId, StepExecutionId},
  integration::{
    Cleanup, FolderListing, KeepReason, LeftBranch, LeftWorktree, Leftovers, WorktreeOwner,
    removal::{Discard, RemovalFailure, RemovalRequest},
  },
  run::BranchName,
};
use orchester_ipc::{
  Command, Event, PROTOCOL_VERSION, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter},
};

fn worktree(path: &str, owner: WorktreeOwner) -> LeftWorktree {
  LeftWorktree {
    path: path.to_owned(),
    owner,
    branch: Some(BranchName::new("orc-1-fix").expect("a branch name")),
    uncommitted: vec!["notes.md".to_owned(), "\"quoted\" é.txt".to_owned()],
    missing: false,
    forgotten: true,
    listing: FolderListing::Unreadable {
      path: "a/b".to_owned(),
      detail: "Access is denied.".to_owned(),
    },
  }
}

const _: () = assert!(
  PROTOCOL_VERSION >= 45,
  "the removal through the runner came with protocol 45"
);

#[tokio::test]
async fn frames_round_trip_a_worktree_removal_and_every_answer() {
  let (near, far) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(far), FrameWriter::new(near));
  let run = RunId::generate();
  let step = WorktreeOwner::Step(StepExecutionId::generate());
  let seen = Leftovers {
    worktrees: vec![worktree("C:/repo/.orchester/worktrees/a", WorktreeOwner::Run)],
    branches: vec![LeftBranch {
      name: BranchName::new("orc-1-fix").expect("a branch name"),
      merged: false,
    }],
    borrows: true,
  };
  let requests = [
    RemovalRequest {
      discard: Discard::Seen(Some(seen)),
      retry_held: true,
    },
    RemovalRequest {
      discard: Discard::Seen(None),
      retry_held: true,
    },
    RemovalRequest {
      discard: Discard::Everything,
      retry_held: false,
    },
  ]
  .map(|request| Request {
    id: RequestId::new(9),
    command: Command::RemoveWorktrees { run, request },
  });
  let branch = || BranchName::new("orc-1-fix--review-1").expect("a branch name");
  let cleanup = Cleanup {
    removed: vec![worktree("/repo/.orchester/worktrees/b", WorktreeOwner::Run)],
    deleted: vec![branch()],
    kept_worktrees: vec![
      (
        worktree("/repo/w/c", step),
        KeepReason::Uncommitted(vec!["x".to_owned()]),
      ),
      (
        worktree("/repo/w/d", step),
        KeepReason::Failed("Directory not empty".to_owned()),
      ),
    ],
    kept_branches: vec![
      (branch(), KeepReason::NotMerged),
      (branch(), KeepReason::InKeptWorktree),
    ],
  };
  let failures = [
    RemovalFailure::LiveWork,
    RemovalFailure::LocalIntegration,
    RemovalFailure::GitMissing,
    RemovalFailure::GitStopped("the program did not finish in time".to_owned()),
    RemovalFailure::GitRefused("fatal: 'w' is locked".to_owned()),
    RemovalFailure::RunUnreadable("no such run".to_owned()),
  ];
  let replies: Vec<Event> = std::iter::once(Reply::WorktreesRemoved(cleanup))
    .chain(failures.into_iter().map(Reply::RemovalFailed))
    .map(|reply| Event::Reply {
      id: RequestId::new(9),
      reply,
    })
    .collect();
  for request in &requests {
    writer.write(request).await.expect("write a request");
  }
  for reply in &replies {
    writer.write(reply).await.expect("write a reply");
  }
  drop(writer);
  for request in requests {
    assert_eq!(reader.read::<Request>().await.expect("read a request"), Some(request));
  }
  for reply in replies {
    assert_eq!(reader.read::<Event>().await.expect("read a reply"), Some(reply));
  }
  assert_eq!(reader.read::<Event>().await.expect("a clean end"), None);
}
