//! The coordinator link's approvals and questions areas (`docs/architecture/remote-runners.md`, "Rules of the
//! boundary"): each request and answer comes back the same from its JSON, so a remote coordinator can answer it, and
//! JSON that breaks a rule a stored rule or question keeps is refused. Run with
//! `cargo test -p orchester-runner --test coordinator_link_approvals`.

use std::fmt::Debug;

use orchester_core::{
  id::{ChatId, InvocationId, MessageId, RunId, StepExecutionId},
  orchester_tools::{OrchesterTool, SERVER},
  orchestrator::{FromOrchestrator, SentWay},
  permission::CommandPrefix,
  question::{AskedQuestions, GivenAnswer, Question, QuestionOption, QuestionRequest, QuestionState, QuestionTurn},
  run::{
    Answerer, ApprovalRule, ApprovalRuleId, CommandShell, NativeCommandAsk, PendingToolApproval, ProjectRule,
    ProjectRuleId, ProjectScope, RuleScope,
    review::{Review, ReviewFailure},
  },
  time::Timestamp,
};
use orchester_runner::execution::link::{
  approvals::{
    ApprovalRequest, AutoApproveRequest, PendingApprovalsRequest, ProjectRulesRequest, RunRulesRequest,
  },
  questions::{QuestionRequestById, QuestionsRequest},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// `value`, written as JSON and read back.
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
  let json = serde_json::to_string(value).expect("the value serializes");
  let back: T = serde_json::from_str(&json).expect("the JSON reads back");
  assert_eq!(&back, value, "{json}");
}

fn at(seconds: i64) -> Timestamp {
  Timestamp::from_unix_millis(1_780_000_000_000 + seconds * 1000)
}

fn qa_scope(script: &str) -> RuleScope {
  RuleScope::SimilarMcpTool {
    server: SERVER.to_owned(),
    tool: OrchesterTool::StartQaApp.as_str().to_owned(),
    folder: None,
    check: Some(script.to_owned()),
  }
}

#[test]
fn coordinator_link_approval_requests_round_trip() {
  let run = RunId::generate();
  round_trip(&RunRulesRequest { run });
  round_trip(&ProjectRulesRequest);
  round_trip(&AutoApproveRequest { run });
  round_trip(&PendingApprovalsRequest { run });
  round_trip(&ApprovalRequest {
    run,
    id: "invocation:call-7".to_owned(),
  });
}

#[test]
fn coordinator_link_approval_answers_round_trip() {
  let run = RunId::generate();
  let similar = ApprovalRule {
    id: ApprovalRuleId::new(3),
    run,
    scope: RuleScope::SimilarMcpTool {
      server: "github".to_owned(),
      tool: "list_issues".to_owned(),
      folder: None,
      check: None,
    },
    created_at: at(1),
    revoked_at: Some(at(2)),
    answered: 4,
  };
  let always = ApprovalRule {
    id: ApprovalRuleId::new(4),
    scope: RuleScope::AlwaysCommand {
      prefix: CommandPrefix::new("cargo test").expect("a prefix"),
    },
    revoked_at: None,
    ..similar
  };
  round_trip(&vec![similar, always]);
  let scope = ProjectScope::stored(qa_scope("0123456789abcdef")).expect("a QA start's scope");
  round_trip(&vec![ProjectRule {
    id: ProjectRuleId::new(1),
    scope,
    made_in: run,
    made_by: "Implement".to_owned(),
    created_at: at(3),
    revoked_at: None,
    answered: 2,
  }]);
  for chosen in [Some(true), Some(false), None] {
    round_trip(&chosen);
  }
  let command = PendingToolApproval {
    id: "invocation:call-1".to_owned(),
    run,
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Implement".to_owned(),
    command: true,
    scope: Some("runs the tests".to_owned()),
    server: "command".to_owned(),
    action: "cargo test -p orchester-core".to_owned(),
    target: Some("H:/worktree".to_owned()),
    requested_at: at(4),
    native: Some(NativeCommandAsk {
      shell: Some(CommandShell::Pwsh),
      time_limit_seconds: 600,
    }),
  };
  let tool = PendingToolApproval {
    id: "invocation:call-2".to_owned(),
    command: false,
    scope: Some(r#"{"path":"README.md"}"#.to_owned()),
    server: "github".to_owned(),
    action: "create_issue".to_owned(),
    target: None,
    native: None,
    ..command.clone()
  };
  round_trip(&vec![command, tool]);
  round_trip(&true);
  for review in [
    Some(Review::Approved {
      reason: "Reads the worktree only.".to_owned(),
      model: "claude-sonnet-5-5".to_owned(),
    }),
    Some(Review::Failed {
      failure: ReviewFailure::TimedOut,
      model: None,
      detail: None,
    }),
    None,
  ] {
    round_trip(&review);
  }
}

#[test]
fn coordinator_link_refuses_a_project_rule_another_rule_scope_names() {
  let scope = ProjectScope::stored(qa_scope("0123456789abcdef")).expect("a QA start's scope");
  let mut json = serde_json::to_value(&scope).expect("the scope serializes");
  json["tool"] = json!("run_check");
  assert!(serde_json::from_value::<ProjectScope>(json).is_err());
  assert!(serde_json::from_value::<ProjectScope>(serde_json::to_value(qa_scope("")).expect("a scope")).is_err());
}

fn layout_question() -> Question {
  let options = vec![
    QuestionOption::new("Sidebar", Some("Navigation on the left")),
    QuestionOption::new("Tabs", None),
  ];
  Question::new("Which layout?", Some("Layout"), options, false).expect("a question")
}

fn request(run: RunId, turn: QuestionTurn) -> QuestionRequest {
  QuestionRequest::new(
    "invocation:ask-1".to_owned(),
    (run, StepExecutionId::generate(), InvocationId::generate()),
    "Implement".to_owned(),
    vec![layout_question()],
    (at(5), turn),
  )
  .expect("one question is asked")
}

fn answered(run: RunId) -> AskedQuestions {
  let request = request(run, QuestionTurn::StepEnd);
  let answers = request
    .answer(&[GivenAnswer {
      choices: vec!["Tabs".to_owned()],
      text: None,
    }])
    .expect("an answer the question takes");
  AskedQuestions {
    request,
    state: QuestionState::Answered {
      answers,
      by: Answerer::Automation,
      at: at(6),
    },
    message: Some(MessageId::generate()),
    orchestrator: Some(FromOrchestrator::new(ChatId::generate(), SentWay::Direct)),
  }
}

#[test]
fn coordinator_link_questions_round_trip_where_each_stands() {
  let run = RunId::generate();
  round_trip(&QuestionsRequest { run });
  round_trip(&QuestionRequestById {
    run,
    id: "invocation:ask-1".to_owned(),
  });
  let waiting = AskedQuestions {
    request: request(run, QuestionTurn::InTurn),
    state: QuestionState::Waiting,
    message: None,
    orchestrator: None,
  };
  let interrupted = AskedQuestions {
    state: QuestionState::Interrupted { at: at(7) },
    ..waiting.clone()
  };
  round_trip(&vec![waiting, interrupted, answered(run)]);
  round_trip(&None::<AskedQuestions>);
}

/// `asked` as JSON, changed by `change`, read back.
fn changed(asked: &AskedQuestions, change: impl FnOnce(&mut Value)) -> Result<AskedQuestions, serde_json::Error> {
  let mut json = serde_json::to_value(asked).expect("the request serializes");
  change(&mut json);
  serde_json::from_value(json)
}

#[test]
fn coordinator_link_refuses_questions_a_stored_request_never_holds() {
  let asked = answered(RunId::generate());
  assert!(
    changed(&asked, |json| json["request"]["questions"] = json!([])).is_err(),
    "a request asks a question"
  );
  assert!(
    changed(&asked, |json| {
      json["state"]["Answered"]["answers"] = json!([{ "choices": ["Columns"], "text": null }]);
    })
    .is_err(),
    "an answer picks an option its question suggests"
  );
  assert!(
    changed(&asked, |json| json["state"] = json!("Waiting")).is_err(),
    "only an answered request names the Orchestrator chat"
  );
}
