//! The records of a step's work, warm workspaces, worktree removals and base notices (`StepRecord`,
//! `WarmWorkspaceRecord`, `RemovalRecord`, `NoticeRecord`), the values the runner's executor sends to the
//! coordinator's sink (ORC-681): each kind survives serialization unchanged, and written as it arrived it stores what
//! the direct write stores, with the store's own rules still in force: an execution keeps its first output, a borrow
//! takes a free workspace, a running run's removal is refused, and a notice is delivered once.
//! `cargo test -p orchester-store --test step_records step_record`.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  base_move::{BaseNotice, NoticeDelivery},
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, PlanLimits},
  },
  id::NoticeId,
  integration::{NotRemoved, RemovalStart},
  resource::ModelId,
  revision::Revision,
  run::{
    Invocation, PendingToolApproval, StepOutcome, StepOutput, StepProgress, TokenUsage,
    review::{ReviewEnd, ReviewRecord},
  },
  store::{ActivityStore, WorkspaceStore, WorktreeStore},
  workspaces::{Borrow, Holder, Stranded, WorkspaceNumber},
};
use orchester_store::{NoticeRecord, ProjectDb, RemovalRecord, StepRecord, WarmWorkspaceRecord, WorkspaceWritten};
use serde::{Serialize, de::DeserializeOwned};

use self::support::{at, model, started_run};

/// `record` sent as JSON and read back, as a sink in another process would receive it.
fn sent<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(record: &T) -> T {
  let text = serde_json::to_string(record).expect("a record serializes");
  let received: T = serde_json::from_str(&text).expect("a record deserializes");
  assert_eq!(&received, record, "the record survives being sent");
  received
}

fn limits() -> PlanLimits {
  PlanLimits {
    windows: Vec::new(),
    plan: Some("pro".to_owned()),
    credits: None,
    blocked: None,
  }
}

/// A waiting request of `invocation`'s agent to run a command, which a review is about.
fn request(db: &mut ProjectDb, invocation: &Invocation) -> PendingToolApproval {
  let approval = PendingToolApproval {
    id: format!("{}:call_1", invocation.id),
    run: invocation.run,
    execution: invocation.execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo build".to_owned(),
    target: None,
    requested_at: at(22),
    native: None,
  };
  db.request_tool_approval(&approval).expect("request");
  approval
}

#[test]
fn step_record_of_outputs_progress_reviews_and_limits_stores_what_direct_writes_store() {
  let (_repo, mut db, run, execution) = started_run();
  let step = run.execution(execution).expect("the run's execution").step().clone();
  let output = StepOutput {
    run: run.id(),
    execution,
    step,
    outcome: Some(StepOutcome::Done),
    text: "Done.".to_owned(),
    unverified: Vec::new(),
    created_at: at(10),
  };
  db.write_step(&sent(&StepRecord::output(&output).expect("a record")))
    .expect("written");
  let later = StepOutput {
    text: "Again.".to_owned(),
    ..output.clone()
  };
  db.write_step(&sent(&StepRecord::output(&later).expect("a record")))
    .expect("written");
  assert_eq!(
    db.step_outputs(run.id()).expect("read"),
    std::slice::from_ref(&output),
    "the first output stays"
  );
  let question = StepOutput {
    outcome: Some(StepOutcome::Question),
    ..output
  };
  assert!(StepRecord::output(&question).is_err(), "a question is no output");

  let line = StepProgress::new("running the tests").expect("a line");
  db.write_step(&sent(&StepRecord::progress(execution, &line, at(20))))
    .expect("written");
  assert_eq!(db.step_progress(execution).expect("read"), Some((line, at(20))));

  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(21))
    .expect("invocation");
  let approval = request(&mut db, &invocation);
  let review = ReviewRecord {
    run: run.id(),
    execution,
    invocation: invocation.id,
    approval: approval.id,
    provider: AgentProvider::ClaudeCodeCli,
    account: None,
    connection: None,
    model: ModelId::new("claude-haiku-4-5").expect("model"),
    started_at: at(25),
    ended_at: at(30),
    usage: Some(TokenUsage {
      input: 200,
      cached_input: 50,
      output: 7,
    }),
    end: ReviewEnd::Approved,
  };
  db.write_step(&sent(&StepRecord::review(&review).expect("a record")))
    .expect("written");
  assert_eq!(db.reviewer_usage(run.id()).expect("read"), [review]);

  let reading = LimitReading {
    provider: AgentProvider::CodexCli,
    account: None,
    at: at(40),
    limits: limits(),
  };
  db.write_step(&sent(&StepRecord::limits(run.id(), &reading).expect("a record")))
    .expect("written");
  assert_eq!(db.last_limit_reading(run.id()).expect("read"), Some(reading));
}

#[test]
fn step_record_of_warm_workspaces_borrows_strands_and_gives_back() {
  let (_repo, mut db, run, _execution) = started_run();
  let holder = Holder::Run { run: run.id() };
  let two = NonZeroU32::new(2).expect("two");
  let borrowed = db
    .write_workspace(&sent(&WarmWorkspaceRecord::borrow(
      holder,
      Some(run.branch()),
      two,
      at(10),
    )))
    .expect("written");
  assert_eq!(
    borrowed,
    WorkspaceWritten::Borrowed(Borrow::Take(WorkspaceNumber::FIRST))
  );
  let stranded = Stranded {
    run: Some(run.id()),
    files: 3,
  };
  assert_eq!(
    db.write_workspace(&sent(&WarmWorkspaceRecord::stranded(
      WorkspaceNumber::FIRST,
      Some(stranded)
    )))
    .expect("written"),
    WorkspaceWritten::Changed
  );
  assert_eq!(
    db.write_workspace(&sent(&WarmWorkspaceRecord::give_back(holder, at(20))))
      .expect("written"),
    WorkspaceWritten::Returned(Some(WorkspaceNumber::FIRST))
  );
  let records = db.workspaces().expect("read");
  assert_eq!(records.len(), 1);
  assert_eq!(records[0].holder, None);
  assert_eq!(records[0].stranded, Some(stranded));
  let one = NonZeroU32::new(1).expect("one");
  let waits = db
    .write_workspace(&sent(&WarmWorkspaceRecord::borrow(holder, None, one, at(30))))
    .expect("written");
  assert!(
    matches!(waits, WorkspaceWritten::Borrowed(Borrow::Wait(_))),
    "a stranded workspace is not taken"
  );
  assert_eq!(db.workspace_waits().expect("read"), [(run.id(), at(30))]);
  db.write_workspace(&sent(&WarmWorkspaceRecord::wait_ended(run.id())))
    .expect("written");
  assert!(db.workspace_waits().expect("read").is_empty());
}

#[test]
fn step_record_of_worktree_removals_keeps_the_store_rules() {
  let (_repo, mut db, run, _execution) = started_run();
  assert_eq!(
    db.write_removal(&sent(&RemovalRecord::begun(run.id(), at(10))))
      .expect("written"),
    Some(RemovalStart::LiveWork),
    "a running run's worktrees are not removed"
  );
  assert_eq!(db.worktree_removal(run.id()).expect("read"), None);
  assert_eq!(
    db.write_removal(&sent(&RemovalRecord::found(run.id(), at(11))))
      .expect("written"),
    None
  );
  assert_eq!(db.worktree_removal(run.id()).expect("read"), Some(at(11)));
  db.write_removal(&sent(&RemovalRecord::undone(run.id(), at(11))))
    .expect("written");
  assert_eq!(db.worktree_removal(run.id()).expect("read"), None);
  let left = NotRemoved {
    path: "C:/work/.orchester/worktrees/run".to_owned(),
    reason: Some("in use".to_owned()),
  };
  db.write_removal(&sent(&RemovalRecord::not_removed(run.id(), &left, at(12))))
    .expect("written");
  assert_eq!(db.worktree_not_removed(run.id()).expect("read"), Some(left));
  db.write_removal(&sent(&RemovalRecord::all_removed(run.id())))
    .expect("written");
  assert_eq!(db.worktree_not_removed(run.id()).expect("read"), None);
}

#[test]
fn step_record_of_base_notices_applies_while_the_notice_is_pending() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(9))
    .expect("invocation");
  let revision = Revision::new(&"c".repeat(40)).expect("revision");
  let notice = BaseNotice {
    id: NoticeId::generate(),
    run: run.id(),
    task: run.task(),
    branch: run.branch().clone(),
    revision,
    commits: Vec::new(),
    tasks: Vec::new(),
    overlapping: vec!["src/b.rs".to_owned()],
    noticed_at: at(5),
    deliver_after: at(5),
    delivery: NoticeDelivery::Pending,
  };
  db.queue_base_notice(&notice).expect("queued");
  let files = ["src/a.rs".to_owned()];
  assert!(
    db.write_notice(&sent(&NoticeRecord::overlap(&notice, &files).expect("a record")))
      .expect("written")
  );
  let pending = db.pending_base_notice(run.id()).expect("read").expect("pending");
  assert_eq!(pending.overlapping, ["src/a.rs", "src/b.rs"]);
  let delivered = NoticeRecord::delivered(&notice, (execution, invocation.id), at(10));
  assert!(db.write_notice(&sent(&delivered)).expect("written"));
  assert!(
    !db.write_notice(&sent(&delivered)).expect("written"),
    "a notice is delivered once"
  );
  let merged = Revision::new(&"d".repeat(40)).expect("revision");
  assert!(
    !db
      .write_notice(&sent(&NoticeRecord::merged(&notice, (execution, &merged), at(11))))
      .expect("written"),
    "a delivered notice is not merged"
  );
  assert_eq!(db.pending_base_notice(run.id()).expect("read"), None);
}
