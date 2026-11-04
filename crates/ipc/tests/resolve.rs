//! Resolving by hand in a run's worktree through the runner (protocol 44): every request and answer, and a failure with
//! git's words, cross the connection as they were sent.

use orchester_core::{
  id::RunId,
  integration::resolve::{
    ConflictFile, FileState, MergedIn, Resolution, Side, Started, Undecided,
    request::{FailureReason, ResolveAnswer, ResolveFailure, ResolveRequest},
  },
  revision::Revision,
  run::BranchName,
};
use orchester_ipc::{
  Command, Event, PROTOCOL_VERSION, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter},
};

fn revision(hex: char) -> Revision {
  Revision::new(&hex.to_string().repeat(40)).expect("a commit id")
}

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a branch name")
}

fn requests() -> Vec<ResolveRequest> {
  vec![
    ResolveRequest::Read,
    ResolveRequest::Present,
    ResolveRequest::InMerge,
    ResolveRequest::Head,
    ResolveRequest::Start {
      base: branch("main"),
      branch: branch("orchester/orc-1-run-1"),
      expected: revision('a'),
    },
    ResolveRequest::Decide {
      path: "assets/logo[1].png".to_owned(),
      side: Side::Base,
    },
    ResolveRequest::Finish {
      base: branch("main"),
      expected: revision('b'),
    },
    ResolveRequest::Abort,
    ResolveRequest::Marked {
      files: vec!["src/lib.rs".to_owned()],
    },
  ]
}

fn replies() -> Vec<Reply> {
  let merge = Resolution {
    files: vec![
      ConflictFile {
        path: "src/lib.rs".to_owned(),
        state: FileState::Markers,
      },
      ConflictFile {
        path: "assets/logo.png".to_owned(),
        state: FileState::Undecided(Undecided::DeletedByBase),
      },
      ConflictFile {
        path: "README.md".to_owned(),
        state: FileState::ResolvedWithMarkers,
      },
    ],
  };
  let merged = MergedIn {
    base: revision('c'),
    tip: revision('d'),
  };
  vec![
    Reply::Resolved(ResolveAnswer::Read {
      present: true,
      merge: Some(merge),
    }),
    Reply::Resolved(ResolveAnswer::Present(false)),
    Reply::Resolved(ResolveAnswer::InMerge(true)),
    Reply::Resolved(ResolveAnswer::Head(revision('e'))),
    Reply::Resolved(ResolveAnswer::Started(Started::Conflicts(vec![
      "src/lib.rs".to_owned(),
    ]))),
    Reply::Resolved(ResolveAnswer::Started(Started::Merged(merged.clone()))),
    Reply::Resolved(ResolveAnswer::Finished(merged)),
    Reply::Resolved(ResolveAnswer::Done),
    Reply::Resolved(ResolveAnswer::Marked(vec!["src/lib.rs".to_owned()])),
    Reply::ResolveFailed(ResolveFailure {
      reason: FailureReason::GitRefused("fatal: not a git repository".to_owned()),
      detail: "git failed: fatal: not a git repository".to_owned(),
    }),
    Reply::ResolveFailed(ResolveFailure {
      reason: FailureReason::MarkersLeft(vec!["src/lib.rs".to_owned()]),
      detail: "conflict markers are left in src/lib.rs".to_owned(),
    }),
  ]
}

#[tokio::test]
async fn resolve_requests_and_answers_round_trip_under_protocol_44() {
  const { assert!(PROTOCOL_VERSION >= 44) };
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (read, _) = tokio::io::split(left);
  let (_, write) = tokio::io::split(right);
  let (mut reader, mut writer) = (FrameReader::new(read), FrameWriter::new(write));
  let run = RunId::generate();
  let sent: Vec<Request> = requests()
    .into_iter()
    .enumerate()
    .map(|(index, request)| Request {
      id: RequestId::new(index as u64 + 1),
      command: Command::Resolve { run, request },
    })
    .collect();
  let answered: Vec<Event> = replies()
    .into_iter()
    .enumerate()
    .map(|(index, reply)| Event::Reply {
      id: RequestId::new(index as u64 + 1),
      reply,
    })
    .collect();
  for request in &sent {
    writer.write(request).await.expect("write request");
  }
  for event in &answered {
    writer.write(event).await.expect("write reply");
  }
  drop(writer);
  for request in sent {
    assert_eq!(reader.read::<Request>().await.expect("read request"), Some(request));
  }
  for event in answered {
    assert_eq!(reader.read::<Event>().await.expect("read reply"), Some(event));
  }
}
