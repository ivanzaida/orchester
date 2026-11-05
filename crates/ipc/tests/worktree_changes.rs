//! A run's worktree changes asked of the runner and its answers (protocol 40) travel as frames unchanged: every state
//! of the read, a failed one included, and the diff's text as git printed it.

use orchester_core::{diff::Changes, id::RunId, revision::Revision};
use orchester_ipc::{
  Command, Event, Reply, Request, RequestId, WorktreeChangesRead, WorktreeReadFailure,
  frame::{FrameReader, FrameWriter},
};

#[tokio::test]
async fn frames_round_trip_a_worktree_read_and_every_answer() {
  let (near, far) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(far), FrameWriter::new(near));
  let run = RunId::generate();
  let head = Revision::new("1111111111111111111111111111111111111111").expect("a revision");
  let request = Request {
    id: RequestId::new(7),
    command: Command::ReadWorktreeChanges { run },
  };
  let reads = [
    WorktreeChangesRead::NoWorktree,
    WorktreeChangesRead::Clean,
    WorktreeChangesRead::Read {
      head: head.clone(),
      diff: "diff --git a/notes.md b/notes.md\nnew file mode 100644\n--- /dev/null\n+++ b/notes.md\n@@ -0,0 +1 @@\n+\"quoted\" \\ é\n"
        .to_owned(),
    },
    WorktreeChangesRead::TooLarge {
      head,
      size: Changes {
        files: 301,
        additions: 20_001,
        deletions: 3,
      },
    },
    WorktreeChangesRead::Failed(WorktreeReadFailure::GitMissing),
    WorktreeChangesRead::Failed(WorktreeReadFailure::Git {
      detail: "fatal: not a git repository".to_owned(),
    }),
  ];
  let replies: Vec<Event> = reads
    .into_iter()
    .map(|read| Event::Reply {
      id: RequestId::new(7),
      reply: Reply::WorktreeChanges { run, read },
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
