//! The migration that rebuilds `tool_approvals` for `decided_by = 'permissions'`: a populated database
//! of the schema before it opens with every request as it was (each state, each answer of that schema,
//! a rule's answer, native commands, a pending request), its index and triggers in place and no
//! dangling reference.
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
  let mut query = database
    .prepare("SELECT * FROM tool_approvals ORDER BY id")
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

#[test]
fn permitted_migration_keeps_every_request_of_the_schema_before() {
  let (repo, asked) = populated();
  let current = version(&database(repo.path()));
  let old = copied_at_version(&repo.path().join(".orchester").join("orchester.db"), current - 1);
  let before = rows(&database(old.path()));
  let states: Vec<&Value> = before.1.iter().map(|row| &row[9]).collect();
  for state in ["approved", "denied", "interrupted", "pending"] {
    assert!(states.contains(&&Value::Text(state.to_owned())), "a {state} request");
  }
  assert_eq!(before.1.len(), 10);
  let indexes = schema_of(&database(old.path()), "index");
  let triggers = schema_of(&database(old.path()), "trigger");

  ProjectDb::open(old.path()).expect("migrate");
  let after = database(old.path());
  assert_eq!(version(&after), current);
  assert_eq!(rows(&after), before);
  assert_eq!(schema_of(&after, "index"), indexes);
  assert_eq!(schema_of(&after, "trigger"), triggers);
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
  let dangling: i64 = after
    .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
    .expect("foreign key check");
  assert_eq!(dangling, 0);
  // The rebuilt table keeps refusing a second decision and takes the new answer.
  let decided = format!("{}:person", asked.invocation);
  assert!(
    after
      .execute("UPDATE tool_approvals SET state = 'denied' WHERE id = ?1", [&decided])
      .is_err()
  );
  let mut db = ProjectDb::open(old.path()).expect("open");
  let permitted = asked.request("permitted", true, None);
  db.request_tool_approval(&permitted).expect("request");
  assert!(
    db.claim_tool_approval(asked.run, &permitted.id, DecidedBy::Permissions, at(13))
      .expect("claimed by the permissions")
  );
}
