//! An agent whose CLI reported input but no output yet, as Claude Code on DeepSeek's Anthropic API does before its
//! result: the Usage chart's footnote says how many such runs' output is unknown, and the run's *Tokens* fact says its
//! output is not reported yet, never a false 0.

use std::path::Path;

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  connection::stream::AgentEvent,
  run::{InvocationKind, TokenUsage},
  store::{ActivityStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::usage::UsageQuery;

use crate::harness::Seeded;

/// The `usage` seed's task whose Implement agent is still at work.
const AT_WORK: &str = "Add retry to webhook sender";

/// Records a turn of `AT_WORK`'s open agent invocation with input but output 0; answers the task's key.
fn output_unreported_so_far(repo: &Path) -> String {
  let mut db = ProjectDb::open(repo).expect("the database opens");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == AT_WORK)
    .expect("the seeded task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let mut invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none() && matches!(invocation.kind, InvocationKind::Agent { .. }))
    .expect("the agent at work");
  let usage = TokenUsage {
    input: 60_000,
    cached_input: 50_000,
    output: 0,
  };
  db.append_activity(&invocation, &[(Timestamp::now(), AgentEvent::TurnCompleted { usage })])
    .expect("the turn is recorded");
  invocation.usage = usage;
  db.save_invocation(&invocation).expect("the invocation is saved");
  task.key().to_string()
}

#[test]
fn usage_unreported_output_is_said_unknown_in_the_footnote_and_the_runs_tokens() {
  let mut plain = Seeded::usage("usage", UsageQuery::default());
  assert!(
    !plain.text("usage-tokens-footnote").contains("no output yet"),
    "every CLI reported its output"
  );
  let mut key = String::new();
  let mut page = Seeded::prepared("usage", false, |repo| key = output_unreported_so_far(repo));
  page.go(UsageQuery::default());
  page.until("the usage", |page| page.shows("usage-tokens-chart"));
  let footnote = page.text("usage-tokens-footnote");
  assert!(
    footnote.ends_with(
      "Agent runs whose CLI reported input but no output yet: 1. Their output is unknown, so Spent and Output leave it out."
    ),
    "{footnote}"
  );
  let summary = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary));
  page.window.navigation().go(Route::Project(page.project, summary));
  page.until("the run's facts", |page| page.shows("task-fact-tokens"));
  let tokens = page.text("task-fact-tokens");
  assert!(tokens.ends_with(" · output not reported yet"), "{tokens}");
}
