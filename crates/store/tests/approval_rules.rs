mod support;

use orchester_core::{
  conversation::{Message, MessageBody, Recipient, Timing},
  id::{ConnectionId, IdempotencyKey},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, Slug, VersionNumber},
  run::{
    ApprovalSubject, DecidedBy, PendingToolApproval, ProjectScope, RuleScope, ToolApprovalDecision, ToolReach,
    review::Review,
  },
  store::{ActivityStore, ConversationStore, ResourceStore, RunStore},
};
use orchester_store::{DecisionCounts, ProjectDb, StoreError};
use support::{at, copied_at_version, model, name, started_run};

/// The schema version before approval choices: who answered a request, approval rules and a run's own
/// auto-approval choice. Migrations are append-only; ORC-61's steering came before.
const BEFORE_APPROVAL_CHOICES: u32 = 37;

fn mcp_rule(tool: &str) -> RuleScope {
  RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: tool.to_owned(),
    folder: None,
    check: None,
  }
}

#[test]
fn approval_rules_are_kept_revoked_and_counted() {
  let (_repo, mut db, run, _) = started_run();
  let rule = db
    .add_approval_rule(run.id(), &mcp_rule("batch_design"), at(10))
    .expect("rule");
  let again = db
    .add_approval_rule(run.id(), &mcp_rule("batch_design"), at(11))
    .expect("same rule");
  assert_eq!(again.id, rule.id);
  let always = RuleScope::AlwaysCommand {
    prefix: CommandPrefix::new("cargo add").expect("prefix"),
  };
  let kept = db.add_approval_rule(run.id(), &always, at(12)).expect("always rule");
  assert_eq!(db.approval_rules(run.id()).expect("rules").len(), 2);
  assert!(
    !db
      .revoke_approval_rule(run.id(), kept.id, at(13))
      .expect("not revocable")
  );
  assert!(db.revoke_approval_rule(run.id(), rule.id, at(14)).expect("revoked"));
  assert!(!db.revoke_approval_rule(run.id(), rule.id, at(15)).expect("once"));
  let rules = db.approval_rules(run.id()).expect("rules");
  assert_eq!(rules[0].revoked_at, Some(at(14)));
  assert!(rules[1].is_active());
  // Revoking ends a rule; the next one of the same scope is a new rule.
  let renewed = db
    .add_approval_rule(run.id(), &mcp_rule("batch_design"), at(16))
    .expect("renewed");
  assert_ne!(renewed.id, rule.id);
}

/// The rule *Approve for every run of this project* keeps for `start_qa_app` at the QA script's `blob`.
fn qa_rule(blob: &str) -> ProjectScope {
  ProjectScope::of(&ApprovalSubject::McpTool {
    server: "Orchester".to_owned(),
    tool: "start_qa_app".to_owned(),
    reach: ToolReach::Script { blob: blob.to_owned() },
  })
  .expect("a project rule")
}

#[test]
fn approval_rules_of_the_project_outlive_their_run_and_are_revoked() {
  let (repo, mut db, run, _) = started_run();
  assert!(db.project_approval_rules().expect("none yet").is_empty());
  let rule = db
    .add_project_approval_rule(&qa_rule("3f2a1b9c"), run.id(), at(10))
    .expect("rule");
  let again = db
    .add_project_approval_rule(&qa_rule("3f2a1b9c"), run.id(), at(11))
    .expect("same rule");
  assert_eq!(again.id, rule.id);
  let other = db
    .add_project_approval_rule(&qa_rule("77aa00"), run.id(), at(12))
    .expect("another script");
  assert_ne!(other.id, rule.id);
  db.project_rule_answered(rule.id).expect("answered");
  drop(db);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  let rules = db.project_approval_rules().expect("rules");
  assert_eq!(rules.len(), 2);
  assert_eq!(
    (rules[0].made_in, rules[0].answered, rules[0].scope.script()),
    (run.id(), 1, "3f2a1b9c")
  );
  assert!(db.revoke_project_approval_rule(rule.id, at(13)).expect("revoked"));
  assert!(!db.revoke_project_approval_rule(rule.id, at(14)).expect("once"));
  let rules = db.project_approval_rules().expect("rules");
  assert_eq!(rules[0].revoked_at, Some(at(13)));
  assert!(!rules[0].is_active() && rules[1].is_active());
  // Rules are revoked, never deleted, and keep the scope they were made with.
  let connection = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("raw");
  assert!(connection.execute("DELETE FROM project_approval_rules", []).is_err());
  assert!(
    connection
      .execute("UPDATE project_approval_rules SET scope = '{}'", [])
      .is_err()
  );
  assert!(
    connection
      .execute("UPDATE project_approval_rules SET revoked_at = NULL", [])
      .is_err(),
    "a revoked rule stays revoked"
  );
}

#[test]
fn approval_rules_of_the_project_are_added_once_under_the_write_lock() {
  let (repo, mut db, run, _) = started_run();
  let other = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("raw");
  other.execute_batch("BEGIN IMMEDIATE").expect("another writer holds the lock");
  db.set_busy_handler_for_test(|_| false).expect("no wait for the lock");
  assert!(
    db.add_project_approval_rule(&qa_rule("3f2a1b9c"), run.id(), at(10)).is_err(),
    "the check and the insert wait for the write lock together"
  );
  other.execute_batch("ROLLBACK").expect("released");
  let rule = db
    .add_project_approval_rule(&qa_rule("3f2a1b9c"), run.id(), at(11))
    .expect("rule");
  assert_eq!(db.project_approval_rules().expect("rules"), vec![rule]);
}

#[test]
fn approval_rules_run_auto_approval_choice() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), None);
  db.set_run_auto_approve(run.id(), Some(true)).expect("on");
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(true));
  db.set_run_auto_approve(run.id(), Some(false)).expect("stopped");
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(false));
}

/// Auto mode looked at the reviewer's approval, at the person's first answer, which it asked about, and at
/// the request still waiting.
fn reviewed(db: &mut ProjectDb, approval: &PendingToolApproval, answer: Option<DecidedBy>) {
  let review = match answer {
    Some(DecidedBy::Reviewer) => Review::Approved {
      reason: "It serves the task.".to_owned(),
      model: "claude-haiku-4-5-20251001".to_owned(),
    },
    // The person's first answer, and the request still waiting, which no count of auto mode holds yet.
    Some(DecidedBy::Person) | None => Review::Asked {
      reason: "Not needed for the task.".to_owned(),
      model: "claude-haiku-4-5-20251001".to_owned(),
    },
    _ => return,
  };
  db.record_review(approval.run, &approval.id, &review).expect("review");
  assert_eq!(
    db.tool_approval_review(approval.run, &approval.id).expect("read"),
    Some(review)
  );
}

#[test]
fn approval_rules_decision_counts_by_who_answered() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let rule = db
    .add_approval_rule(run.id(), &mcp_rule("batch_design"), at(10))
    .expect("rule");
  let answers = [
    Some(DecidedBy::Person),
    Some(DecidedBy::PersonSimilar),
    Some(DecidedBy::SimilarRule),
    Some(DecidedBy::AutoApproved),
    Some(DecidedBy::AutoApproved),
    Some(DecidedBy::AlwaysAllowed),
    Some(DecidedBy::Permissions),
    Some(DecidedBy::PersonDeny),
    Some(DecidedBy::Automation),
    Some(DecidedBy::AutomationDeny),
    Some(DecidedBy::Reviewer),
    None,
  ];
  for (number, answer) in answers.into_iter().enumerate() {
    let approval = PendingToolApproval {
      id: format!("{}:{number}", invocation.id),
      run: run.id(),
      execution,
      invocation: invocation.id,
      agent: "Designer".to_owned(),
      command: false,
      scope: None,
      server: "pencil".to_owned(),
      action: "batch_design".to_owned(),
      target: None,
      requested_at: at(11),
      native: None,
    };
    db.request_tool_approval(&approval).expect("request");
    reviewed(&mut db, &approval, answer);
    let Some(answer) = answer else { continue };
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
  assert_eq!(
    db.decision_counts(run.id()).expect("counts"),
    DecisionCounts {
      by_you: 2,
      denied_by_you: 1,
      by_similar_rule: 1,
      always_allowed: 1,
      by_profile: 1,
      auto_approved: 2,
      by_automation: 1,
      denied_by_automation: 1,
      by_reviewer: 1,
      asked_by_reviewer: 1,
      waiting: 1,
      reviewing: 0,
    }
  );
  assert_eq!(db.approval_rules(run.id()).expect("rules")[0].answered, 1);
}

#[test]
fn approval_rules_always_allow_publishes_the_prefix_and_keeps_the_draft() {
  let (repo, mut db, ..) = started_run();
  let listed = |prefixes: &[&str]| {
    let entries = prefixes
      .iter()
      .map(|prefix| CommandPrefix::new(prefix).expect("prefix"))
      .collect();
    AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
      NetworkAccess::On,
    )
  };
  let profile = AgentProfile::new(
    name("Designer"),
    String::new(),
    PromptText::new("Design.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(listed(&["cargo test"]));
  let id = db
    .create_resource(Slug::new("designer").expect("slug"), name("Designer"), &profile, at(1))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, at(2)).expect("v1");
  let edited = profile.clone().with_permissions(listed(&["cargo test", "cargo check"]));
  db.save_draft(id, &Draft::new(edited, Some(VersionNumber::FIRST), at(3)))
    .expect("draft");
  let scope = CommandScope::new(repo.path(), &[], None, None);
  let command = ClassifiedCommand::classify("cargo add serde --features derive", &scope);
  let prefix = CommandPrefix::new("cargo add").expect("prefix");
  let published = db
    .publish_allowed_command(id, &command, &prefix, at(4))
    .expect("published");
  assert_eq!(published.get(), 2);
  let latest = db.latest_version::<AgentProfile>(id).expect("read").expect("v2");
  assert_eq!(latest.body().permissions(), &listed(&["cargo test", "cargo add"]));
  let draft = db.draft::<AgentProfile>(id).expect("read").expect("draft kept");
  assert_eq!(draft.based_on(), Some(published));
  assert_eq!(
    draft.body().permissions(),
    &listed(&["cargo test", "cargo check", "cargo add"])
  );
  let refused = db.publish_allowed_command(id, &command, &CommandPrefix::new("rm").expect("prefix"), at(5));
  assert!(matches!(refused, Err(StoreError::AlwaysAllow(_))));
}

#[test]
fn approval_rules_migration_keeps_runs_and_waiting_requests() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let approval = PendingToolApproval {
    id: format!("{}:1", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "cargo test".to_owned(),
    target: None,
    requested_at: at(11),
    native: None,
  };
  db.request_tool_approval(&approval).expect("request");
  // A message sent now, which only schema 37 (steering) keeps.
  let now = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new("Use the new API.").expect("a message"),
    at(12),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  db.send_message(IdempotencyKey::generate(), &now).expect("sent now");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_APPROVAL_CHOICES);
  let db = ProjectDb::open(old.path()).expect("migrate the schema 37 database");
  assert_eq!(db.messages(run.task()).expect("messages"), [now]);
  assert!(db.run(run.id()).expect("read").is_some());
  // A run that started before follows its profiles' setting.
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), None);
  assert_eq!(db.pending_tool_approvals(run.id()).expect("pending"), [approval]);
  assert!(db.approval_rules(run.id()).expect("rules").is_empty());
  assert_eq!(db.decision_counts(run.id()).expect("counts").waiting, 1);
}

#[test]
fn approval_rules_stop_auto_approving_chooses_and_writes_under_one_write_lock() {
  let (repo, mut db, run, _) = started_run();
  db.set_run_auto_approve(run.id(), Some(true)).expect("auto-approve");
  let path = repo.path().join(".orchester/orchester.db");
  let stopped = db
    .stop_auto_approving(&|_| {
      // Another writer cannot turn a run's auto-approval on between the choice and the write.
      let other = rusqlite::Connection::open(&path).expect("a second connection");
      other.busy_timeout(std::time::Duration::ZERO).expect("no waiting");
      let blocked = other.execute("UPDATE runs SET auto_approve = 1", []);
      assert!(blocked.is_err(), "the write lock is held while the runs are chosen");
      Ok(vec![run.id()])
    })
    .expect("stopped");
  assert_eq!(stopped, [run.id()]);
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(false));
  // A choice that fails changes nothing.
  db.set_run_auto_approve(run.id(), Some(true)).expect("auto-approve");
  let failed = db.stop_auto_approving(&|_| Err(StoreError::RunNotFound(run.id())));
  assert!(failed.is_err());
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(true));
}

#[test]
fn approval_rules_of_the_project_mark_the_answer_that_made_them() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("invocation");
  let target = orchester_core::orchester_tools::QaApproval {
    script: "3f2a1b9c".to_owned(),
    scenario: "tasks".to_owned(),
    changed: Vec::new(),
  }
  .target();
  let request = |id: &str, at_millis: i64| PendingToolApproval {
    id: format!("{}:{id}", invocation.id),
    run: run.id(),
    execution,
    invocation: invocation.id,
    agent: "Verifier".to_owned(),
    command: false,
    scope: None,
    server: "Orchester".to_owned(),
    action: "start_qa_app".to_owned(),
    target: Some(target.clone()),
    requested_at: at(at_millis),
    native: None,
  };
  for (id, when) in [("qa-1", 11), ("qa-2", 12)] {
    db.request_tool_approval(&request(id, when)).expect("request");
    db.claim_tool_approval(run.id(), &request(id, when).id, DecidedBy::PersonSimilar, at(when + 1))
      .expect("claimed");
  }
  let marked = |db: &ProjectDb| -> Vec<bool> {
    db.decided_tool_approvals(run.id())
      .expect("decided")
      .iter()
      .map(|decided| decided.for_project)
      .collect()
  };
  assert_eq!(marked(&db), [false, false], "no rule of the project yet");
  db.add_project_approval_rule(&qa_rule("3f2a1b9c"), run.id(), at(20))
    .expect("rule");
  assert_eq!(marked(&db), [true, true]);
  db.add_project_approval_rule(&qa_rule("77aa00"), run.id(), at(21))
    .expect("another script's rule");
  assert_eq!(marked(&db), [true, true]);
}
