//! Why a review gave no verdict, kept with the request it judged (ORC-587): a failed review keeps its detail, and a
//! review recorded before it was kept still reads, without one.
//! `cargo test -p orchester-store --test review_detail review_detail`.

mod support;

use orchester_core::{
  run::{
    PendingToolApproval,
    review::{Review, ReviewFailure},
  },
  store::ActivityStore,
};
use support::{at, model, started_run};

#[test]
fn review_detail_is_kept_with_a_failed_review_and_older_records_read_without_it() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let request = |id: &str| PendingToolApproval {
    id: format!("{}:{id}", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo build".to_owned(),
    target: None,
    requested_at: at(11),
    native: None,
  };
  let (failed, older) = (request("failed"), request("older"));
  db.request_tool_approval(&failed).expect("request");
  db.request_tool_approval(&older).expect("request");

  let review = Review::Failed {
    failure: ReviewFailure::CouldNotRun,
    model: Some("claude-haiku-4-5-20251001".to_owned()),
    detail: Some("the reviewer could not run: the CLI exited with code 1: Not logged in".to_owned()),
  };
  db.record_review(run.id(), &failed.id, &review).expect("review");
  assert_eq!(
    db.tool_approval_review(run.id(), &failed.id).expect("read"),
    Some(review.clone())
  );
  let shown = serde_json::to_value(&review).expect("JSON");
  assert_eq!(
    shown["detail"], "the reviewer could not run: the CLI exited with code 1: Not logged in",
    "`auto_mode` of list_approvals and get_run is the review as recorded"
  );

  // As a review was stored before its detail was kept.
  let connection = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("raw");
  connection
    .execute(
      "UPDATE tool_approvals SET review = ?2 WHERE id = ?1",
      [
        older.id.as_str(),
        r#"{"schema":1,"body":{"outcome":"failed","failure":"timed_out","model":"claude-haiku-4-5"}}"#,
      ],
    )
    .expect("an older record");
  assert_eq!(
    db.tool_approval_review(run.id(), &older.id).expect("read"),
    Some(Review::Failed {
      failure: ReviewFailure::TimedOut,
      model: Some("claude-haiku-4-5".to_owned()),
      detail: None,
    })
  );
  let none = serde_json::to_value(Review::Failed {
    failure: ReviewFailure::NoReviewer,
    model: None,
    detail: None,
  })
  .expect("JSON");
  assert!(none.get("detail").is_none(), "{none}");
}
