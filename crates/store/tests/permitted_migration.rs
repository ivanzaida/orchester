//! The migrations that rebuild `tool_approvals` for `decided_by = 'permissions'`, then for automation's
//! answers (`'automation'`, `'automation_deny'`), then for auto mode (`'reviewer'` and the `review`
//! column): a populated database of the schema before any of them opens with every request as it was
//! (each state, each answer of that schema, a rule's answer, native commands, a pending request), its
//! index and triggers in place and no dangling reference; one between them keeps its `permissions` or
//! automation answers.
//! `cargo test -p orchester-store --test permitted_migration permitted_migration`.
mod support;

use std::path::Path;

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  run::{CommandShell, DecidedBy, NativeCommandAsk, PendingToolApproval, RuleScope, ToolApprovalDecision},
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use rusqlite::{Connection, types::Value};
use support::{at, copied_at_version, model, started_run};
use tempfile::TempDir;

/// Every column of every request, by id.
type Rows = (Vec<String>, Vec<Vec<Value>>);

fn rows(database: &Connection) -> Rows {
  rows_of(database, "*")
}

/// The columns `columns` (`*` for all) of every request, by id.
fn rows_of(database: &Connection, columns: &str) -> Rows {
  let mut query = database
    .prepare(&format!("SELECT {columns} FROM tool_approvals ORDER BY id"))
    .expect("query");
  let columns = query.column_names().into_iter().map(str::to_owned).collect();
  let count = query.column_count();
  let rows = query
    .query_map([], |row| (0..count).map(|at| row.get::<_, Value>(at)).collect())
    .expect("rows")
    .collect::<Result<_, _>>()
    .expect("rows");
  (columns, rows)
}

/// The names and SQL of the table's indexes or triggers (`kind`), by name.
fn schema_of(database: &Connection, kind: &str) -> Vec<(String, Option<String>)> {
  database
    .prepare("SELECT name, sql FROM sqlite_master WHERE type = ?1 AND tbl_name = 'tool_approvals' ORDER BY name")
    .expect("query")
    .query_map([kind], |row| Ok((row.get(0)?, row.get(1)?)))
    .expect("names")
    .collect::<Result<_, _>>()
    .expect("names")
}

fn version(database: &Connection) -> u32 {
  database
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version")
}

fn database(repo: &Path) -> Connection {
  Connection::open(repo.join(".orchester").join("orchester.db")).expect("open")
}

/// Where the requests are asked: one invocation of a started run.
#[derive(Clone, Copy)]
struct Asked {
  run: RunId,
  execution: StepExecutionId,
  invocation: InvocationId,
}

impl Asked {
  /// Request `id`: a command (with its scope and folder, and `native`'s shell and time limit) or an MCP call.
  fn request(self, id: &str, command: bool, native: Option<NativeCommandAsk>) -> PendingToolApproval {
    PendingToolApproval {
      id: format!("{}:{id}", self.invocation),
      run: self.run,
      execution: self.execution,
      invocation: self.invocation,
      agent: "Builder".to_owned(),
      command,
      scope: command.then(|| "worktree".to_owned()),
      server: if command { "Orchester" } else { "pencil" }.to_owned(),
      action: if command { "cargo test" } else { "batch_design" }.to_owned(),
      target: command.then(|| ".orchester/worktrees/w1".to_owned()),
      requested_at: at(11),
      native,
    }
  }
}

/// A project whose requests hold every state and every answer of the schema before the migration: one
/// per answer (a rule's with its `rule_id`, a person's with a native command's columns), one ended
/// without an answer, one pending with a native shell.
fn populated() -> (TempDir, Asked) {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let asked = Asked {
    run: run.id(),
    execution,
    invocation: invocation.id,
  };
  let scope = RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: "batch_design".to_owned(),
    folder: None,
  };
  let rule = db.add_approval_rule(run.id(), &scope, at(10)).expect("rule");
  let direct = NativeCommandAsk {
    shell: None,
    time_limit_seconds: 1_800,
  };
  let answered = [
    ("person", DecidedBy::Person, true, Some(direct)),
    ("similar", DecidedBy::PersonSimilar, false, None),
    ("always", DecidedBy::PersonAlways, true, None),
    ("run", DecidedBy::PersonRun, true, None),
    ("deny", DecidedBy::PersonDeny, true, None),
    ("rule", DecidedBy::SimilarRule, false, None),
    ("listed", DecidedBy::AlwaysAllowed, true, None),
    ("auto", DecidedBy::AutoApproved, true, None),
  ];
  for (id, answer, command, native) in answered {
    let approval = asked.request(id, command, native);
    db.request_tool_approval(&approval).expect("request");
    assert!(
      db.claim_tool_approval(run.id(), &approval.id, answer, at(12))
        .expect("claim")
    );
    if answer == DecidedBy::SimilarRule {
      db.rule_answered(run.id(), rule.id, &approval.id).expect("answered");
    }
    let decision = if answer.approves() {
      ToolApprovalDecision::Approved
    } else {
      ToolApprovalDecision::Denied
    };
    assert!(
      db.delivered_tool_approval(run.id(), &approval.id, decision)
        .expect("delivered")
    );
  }
  let ended = asked.request("ended", true, None);
  db.request_tool_approval(&ended).expect("request");
  assert!(db.end_waiting_tool_approval(run.id(), &ended.id).expect("ended"));
  let shell = NativeCommandAsk {
    shell: Some(CommandShell::Pwsh),
    time_limit_seconds: 120,
  };
  db.request_tool_approval(&asked.request("pending", true, Some(shell)))
    .expect("pending request");
  (repo, asked)
}

/// The schema versions before the `permissions` rebuild, before the automation rebuild, which follows
/// it, and before the auto mode rebuild, which follows that, of a database at `current`; the auto mode
/// reviewer's columns, the run's recorded origin, the reviewing flag and the native agents' conversations
/// come last.
const fn before_rebuilds(current: u32) -> (u32, u32, u32) {
  (current - 7, current - 6, current - 5)
}

/// Upgrades `old` and checks that its requests, index and triggers are as `old` had them.
fn upgraded(old: &Path, current: u32) -> Connection {
  let before = rows(&database(old));
  let indexes = schema_of(&database(old), "index");
  let triggers = schema_of(&database(old), "trigger");
  ProjectDb::open(old).expect("migrate");
  let after = database(old);
  assert_eq!(version(&after), current);
  // Every column it had keeps its values; auto mode's `review` comes empty.
  assert_eq!(rows_of(&after, &before.0.join(", ")), before);
  let reviewed: i64 = after
    .query_row(
      "SELECT COUNT(*) FROM tool_approvals WHERE review IS NOT NULL",
      [],
      |row| row.get(0),
    )
    .expect("reviews");
  assert_eq!(reviewed, 0);
  assert_eq!(schema_of(&after, "index"), indexes);
  assert_eq!(schema_of(&after, "trigger"), triggers);
  let dangling: i64 = after
    .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
    .expect("foreign key check");
  assert_eq!(dangling, 0);
  after
}

/// The rebuilt table takes every answer that came with the rebuilds.
fn takes_the_new_answers(old: &Path, asked: Asked) {
  let mut db = ProjectDb::open(old).expect("open");
  for (id, answer) in [
    ("permitted", DecidedBy::Permissions),
    ("automation", DecidedBy::Automation),
    ("automation-deny", DecidedBy::AutomationDeny),
    ("reviewer", DecidedBy::Reviewer),
  ] {
    let request = asked.request(id, true, None);
    db.request_tool_approval(&request).expect("request");
    assert!(
      db.claim_tool_approval(asked.run, &request.id, answer, at(13))
        .expect("claimed"),
      "{answer:?}"
    );
  }
}

#[test]
fn permitted_migration_then_automation_keeps_permissions_answers() {
  let (repo, asked) = populated();
  let mut db = ProjectDb::open(repo.path()).expect("open");
  let permitted = asked.request("by-permissions", true, None);
  db.request_tool_approval(&permitted).expect("request");
  assert!(
    db.claim_tool_approval(asked.run, &permitted.id, DecidedBy::Permissions, at(12))
      .expect("claim")
  );
  drop(db);
  let current = version(&database(repo.path()));
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    before_rebuilds(current).1,
  );
  upgraded(old.path(), current);
  takes_the_new_answers(old.path(), asked);
}

#[test]
fn permitted_migration_then_auto_mode_keeps_automation_answers() {
  let (repo, asked) = populated();
  let mut db = ProjectDb::open(repo.path()).expect("open");
  for (id, answer) in [
    ("by-automation", DecidedBy::Automation),
    ("denied-by-automation", DecidedBy::AutomationDeny),
  ] {
    let request = asked.request(id, true, None);
    db.request_tool_approval(&request).expect("request");
    assert!(
      db.claim_tool_approval(asked.run, &request.id, answer, at(12))
        .expect("claim")
    );
  }
  drop(db);
  let current = version(&database(repo.path()));
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    before_rebuilds(current).2,
  );
  upgraded(old.path(), current);
  takes_the_new_answers(old.path(), asked);
}

#[test]
fn permitted_migration_keeps_every_request_of_the_schema_before() {
  let (repo, asked) = populated();
  let current = version(&database(repo.path()));
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    before_rebuilds(current).0,
  );
  let before = rows(&database(old.path()));
  let states: Vec<&Value> = before.1.iter().map(|row| &row[9]).collect();
  for state in ["approved", "denied", "interrupted", "pending"] {
    assert!(states.contains(&&Value::Text(state.to_owned())), "a {state} request");
  }
  assert_eq!(before.1.len(), 10);

  let after = upgraded(old.path(), current);
  let triggers = schema_of(&after, "trigger");
  let names: Vec<&str> = triggers.iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(
    names,
    [
      "command_approvals_metadata_immutable",
      "native_command_approvals_immutable",
      "native_command_approvals_valid",
      "tool_approvals_decided_once",
    ]
  );
  // The rebuilt table keeps refusing a second decision.
  let decided = format!("{}:person", asked.invocation);
  assert!(
    after
      .execute("UPDATE tool_approvals SET state = 'denied' WHERE id = ?1", [&decided])
      .is_err()
  );
  takes_the_new_answers(old.path(), asked);
}
