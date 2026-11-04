//! A run's file listing for the desktop's code viewer travels in pages: a worktree with more files than one frame
//! holds is answered in replies that each fit a frame, and the pages together list every file once, in order.

use orchester_ipc::{
  Event, Reply, RequestId,
  frame::MAX_FRAME_LEN,
  run_files::{MAX_LIST_PAGE_LEN, RunFilesReply},
};
use orchester_runner::execution::list_page;

/// The length of `reply` as the runner writes it to the desktop.
fn wire_len(reply: &RunFilesReply) -> usize {
  let event = Event::Reply {
    id: RequestId::new(u64::MAX),
    reply: Reply::RunFiles(reply.clone()),
  };
  serde_json::to_vec(&event).expect("JSON").len()
}

#[test]
fn run_files_a_listing_larger_than_a_frame_is_answered_in_pages_that_each_fit_one() {
  // About 24 MiB of names, more than a frame holds, with names JSON must escape among them.
  let files: Vec<String> = (0..300_000)
    .map(|n| format!("packages/module-{n:06}/src/components/\"quoted\"\\nested/file-{n:06}.tsx"))
    .collect();
  let whole = serde_json::to_vec(&files).expect("JSON").len();
  assert!(whole > MAX_FRAME_LEN as usize, "the listing outgrows a frame: {whole}");

  let mut listed = Vec::new();
  let mut from = 0;
  let mut pages = 0;
  loop {
    let reply = list_page(files.clone(), from, MAX_LIST_PAGE_LEN);
    assert!(wire_len(&reply) < MAX_FRAME_LEN as usize, "page {pages} fits a frame");
    let RunFilesReply::Listed { files: page, next } = reply else {
      panic!("a listing answers with its page: {reply:?}");
    };
    assert!(!page.is_empty(), "every page lists a file");
    listed.extend(page);
    pages += 1;
    match next {
      Some(next) => from = next,
      None => break,
    }
  }
  assert!(pages > 1, "more than one page");
  assert_eq!(listed, files);
}

#[test]
fn run_files_a_short_listing_is_one_page_and_a_name_over_the_budget_still_travels() {
  let files = vec!["src/main.rs".to_owned(), "notes.md".to_owned()];
  assert_eq!(
    list_page(files.clone(), 0, MAX_LIST_PAGE_LEN),
    RunFilesReply::Listed { files, next: None }
  );

  let long = "a".repeat(64);
  let files = vec![long.clone(), "b".to_owned()];
  assert_eq!(
    list_page(files.clone(), 0, 8),
    RunFilesReply::Listed {
      files: vec![long],
      next: Some(1)
    }
  );
  assert_eq!(
    list_page(files.clone(), 1, 8),
    RunFilesReply::Listed {
      files: vec!["b".to_owned()],
      next: None
    }
  );
  assert_eq!(
    list_page(files, 5, 8),
    RunFilesReply::Listed {
      files: Vec::new(),
      next: None
    }
  );
}
