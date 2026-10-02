mod support;

use orchester_core::{
  id::ConnectionId,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, Slug, VersionNumber},
  run::{DecidedBy, PendingToolApproval, RuleScope, ToolApprovalDecision},
  store::{ActivityStore, ResourceStore},
};
use orchester_store::{DecisionCounts, StoreError};
use support::{at, model, name, started_run};

fn mcp_rule(tool: &str) -> RuleScope {
  RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: tool.to_owned(),
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

#[test]
fn approval_rules_run_auto_approval_choice() {
  let (_repo, mut db, run, _) = started_run();
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), None);
  db.set_run_auto_approve(run.id(), Some(true)).expect("on");
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(true));
  db.set_run_auto_approve(run.id(), Some(false)).expect("stopped");
  assert_eq!(db.run_auto_approve(run.id()).expect("read"), Some(false));
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
    Some(DecidedBy::PersonDeny),
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
      auto_approved: 2,
      waiting: 1,
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
