//! Rules of the project (the person's decision of 2026-10-18): *Approve for every run of this project* is offered only
//! for Orchester's `start_qa_app` naming its script, keeps the rule to that script's git blob, answers a later call of
//! any run with the same blob while it is active, and only a person gives it.
//! `cargo test -p orchester-core --test approval_project_rules project_rules`.

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  orchester_tools::{QaApproval, SERVER},
  run::{
    Answerer, ApprovalAnswer, ApprovalSubject, AutomationAnswerError, DecidedBy, PendingToolApproval, ProjectRule,
    ProjectRuleId, ProjectScope, RuleScope, ToolReach, answering_in_project,
  },
  time::Timestamp,
};

fn qa_call(blob: &str) -> ApprovalSubject {
  ApprovalSubject::McpTool {
    server: SERVER.to_owned(),
    tool: "start_qa_app".to_owned(),
    reach: ToolReach::Script { blob: blob.to_owned() },
  }
}

fn rule(id: i64, subject: &ApprovalSubject, revoked: bool) -> ProjectRule {
  ProjectRule {
    id: ProjectRuleId::new(id),
    scope: ProjectScope::of(subject).expect("a project rule"),
    made_in: RunId::generate(),
    made_by: "invocation:qa".to_owned(),
    created_at: Timestamp::from_unix_millis(1),
    revoked_at: revoked.then(|| Timestamp::from_unix_millis(2)),
    answered: 0,
  }
}

#[test]
fn project_rules_are_offered_only_for_the_qa_app_naming_its_script() {
  let scope = ProjectScope::of(&qa_call("3f2a1b9c0d4e")).expect("a project rule");
  assert_eq!((scope.server(), scope.tool()), (SERVER, "start_qa_app"));
  assert_eq!((scope.script(), scope.short_script()), ("3f2a1b9c0d4e", "3f2a1b9"));
  assert_eq!(
    scope.rule_scope(),
    RuleScope::SimilarMcpTool {
      server: SERVER.to_owned(),
      tool: "start_qa_app".to_owned(),
      folder: None,
      check: Some("3f2a1b9c0d4e".to_owned()),
    }
  );
  assert_eq!(ProjectScope::stored(scope.rule_scope()), Some(scope));
  assert_eq!(ProjectScope::of(&qa_call("")), None, "a call that names no script");
  for (server, tool) in [(SERVER, "run_check"), ("pencil", "start_qa_app"), (SERVER, "get_run")] {
    let subject = ApprovalSubject::McpTool {
      server: server.to_owned(),
      tool: tool.to_owned(),
      reach: ToolReach::Script {
        blob: "3f2a1b9c".to_owned(),
      },
    };
    assert_eq!(ProjectScope::of(&subject), None, "{server} {tool}");
  }
}

#[test]
fn project_rules_are_read_from_the_request_alone() {
  let request = |command: bool, target: &str| PendingToolApproval {
    id: "invocation:1".to_owned(),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Verifier".to_owned(),
    command,
    scope: None,
    server: SERVER.to_owned(),
    action: "start_qa_app".to_owned(),
    target: Some(target.to_owned()),
    requested_at: Timestamp::now(),
    native: None,
  };
  let target = QaApproval {
    script: "3f2a1b9c".to_owned(),
    scenario: "tasks".to_owned(),
    changed: vec!["build.rs".to_owned()],
  }
  .target();
  assert_eq!(
    ProjectScope::of_request(&request(false, &target)).map(|scope| scope.script().to_owned()),
    Some("3f2a1b9c".to_owned())
  );
  assert_eq!(ProjectScope::of_request(&request(false, "a folder")), None);
  assert_eq!(ProjectScope::of_request(&request(true, &target)), None);
}

#[test]
fn project_rules_answer_the_same_script_while_active() {
  let approved = qa_call("3f2a1b9c");
  let rules = [rule(1, &approved, true), rule(2, &approved, false)];
  assert_eq!(
    answering_in_project(&rules, &approved).map(|rule| rule.id),
    Some(ProjectRuleId::new(2))
  );
  assert!(
    answering_in_project(&rules, &qa_call("77aa00")).is_none(),
    "a changed script asks again"
  );
  assert!(
    answering_in_project(&rules[..1], &approved).is_none(),
    "a revoked rule asks again"
  );
  let other = ApprovalSubject::McpTool {
    server: SERVER.to_owned(),
    tool: "run_check".to_owned(),
    reach: ToolReach::Check {
      name: "3f2a1b9c".to_owned(),
      every_call: false,
    },
  };
  assert!(answering_in_project(&rules, &other).is_none());
}

#[test]
fn project_rules_are_made_by_a_person_only() {
  let subject = qa_call("3f2a1b9c");
  assert_eq!(
    ApprovalAnswer::SimilarProject.given_by(Answerer::Person, &subject),
    Ok(DecidedBy::PersonSimilar)
  );
  assert_eq!(
    ApprovalAnswer::SimilarProject.given_by(Answerer::Automation, &subject),
    Err(AutomationAnswerError::Wider)
  );
}
