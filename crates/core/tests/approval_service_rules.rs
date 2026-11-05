//! Approving a service start (`docs/architecture/agent-testing.md`, "Service › approval"): a request names every
//! service a start runs with its program, arguments and variable names, never their values; *Approve for every run*
//! keeps one rule of the project per service, keyed by a fingerprint of its program, arguments, variables and seed,
//! so a change to any of them asks again; a later start is answered only when every service it runs has a rule.
//! `cargo test -p orchester-core --test approval_service_rules service_rules`.

use std::collections::BTreeMap;

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  orchester_tools::SERVER,
  resource::{CommandArg, CommandProgram, VariableName},
  run::{
    ApprovalSubject, PendingToolApproval, ProjectRule, ProjectRuleId, ProjectScope, RuleScope, ToolReach,
    answering_in_project,
  },
  secret::SecretName,
  service::{
    Readiness, ReadyCheck, ReadyTimeout, Seed, Service, ServiceApproval, ServiceName, ServiceValue, ServiceVariables,
    start_target, started_by,
  },
  time::Timestamp,
};

const TOKEN: &str = "plain-token-value-7f3e";

fn args(words: &[&str]) -> Vec<CommandArg> {
  words
    .iter()
    .map(|word| CommandArg::new(word).expect("an argument"))
    .collect()
}

fn variables(pairs: &[(&str, ServiceValue)]) -> ServiceVariables {
  ServiceVariables::new(
    pairs
      .iter()
      .map(|(name, value)| (VariableName::new(name).expect("a name"), value.clone()))
      .collect::<BTreeMap<_, _>>(),
  )
  .expect("variables")
}

/// The service `name` running `node server.js --port {{port}}` with the variables `MODE=dev` and `API_TOKEN` from the
/// secret `SERVICE_TOKEN`.
fn service(name: &str) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(20).expect("in range"));
  Service::new(
    ServiceName::new(name).expect("a name"),
    CommandProgram::new("node").expect("a program"),
    readiness,
  )
  .with_arguments(args(&["server.js", "--port", "{{port}}"]))
  .with_variables(variables(&[
    ("MODE", ServiceValue::Plain("dev".to_owned())),
    (
      "API_TOKEN",
      ServiceValue::Secret(SecretName::new("SERVICE_TOKEN").expect("a name")),
    ),
  ]))
}

fn start(tool: &str, services: &[&Service]) -> ApprovalSubject {
  ApprovalSubject::McpTool {
    server: SERVER.to_owned(),
    tool: tool.to_owned(),
    reach: ToolReach::Services {
      services: services.iter().map(|service| ServiceApproval::of(service)).collect(),
    },
  }
}

fn rule(id: i64, service: &Service, revoked: bool) -> ProjectRule {
  ProjectRule {
    id: ProjectRuleId::new(id),
    scope: ProjectScope::Service(ServiceApproval::of(service)),
    made_in: RunId::generate(),
    made_by: "invocation:start".to_owned(),
    created_at: Timestamp::from_unix_millis(1),
    revoked_at: revoked.then(|| Timestamp::from_unix_millis(2)),
    answered: 0,
  }
}

fn answered_by(rules: &[ProjectRule], subject: &ApprovalSubject) -> Option<Vec<i64>> {
  answering_in_project(rules, subject).map(|rules| rules.iter().map(|rule| rule.id.get()).collect())
}

#[test]
fn service_rules_change_with_the_program_an_argument_a_variable_or_the_seed() {
  let web = service("web");
  let fingerprint = |service: &Service| ServiceApproval::of(service).fingerprint;
  assert_eq!(fingerprint(&web), fingerprint(&service("web")), "the same definition");
  assert_eq!(fingerprint(&web).len(), 64, "a SHA-256 in hex");
  let changed = [
    (
      "program",
      Service::new(
        web.name().clone(),
        CommandProgram::new("deno").expect("a program"),
        web.readiness().clone(),
      )
      .with_arguments(web.arguments().to_vec())
      .with_variables(web.variables().clone()),
    ),
    (
      "an argument",
      web
        .clone()
        .with_arguments(args(&["server.js", "--port", "{{port}}", "--debug"])),
    ),
    (
      "a variable's plain value",
      web.clone().with_variables(variables(&[
        ("MODE", ServiceValue::Plain("prod".to_owned())),
        (
          "API_TOKEN",
          ServiceValue::Secret(SecretName::new("SERVICE_TOKEN").expect("a name")),
        ),
      ])),
    ),
    (
      "a variable's secret",
      web.clone().with_variables(variables(&[
        ("MODE", ServiceValue::Plain("dev".to_owned())),
        (
          "API_TOKEN",
          ServiceValue::Secret(SecretName::new("OTHER_TOKEN").expect("a name")),
        ),
      ])),
    ),
    (
      "a variable's name",
      web
        .clone()
        .with_variables(variables(&[("MODE", ServiceValue::Plain("dev".to_owned()))])),
    ),
    (
      "the seed",
      web.clone().with_seed(Seed::Command {
        program: CommandProgram::new("node").expect("a program"),
        arguments: args(&["seed.js"]),
      }),
    ),
  ];
  for (what, other) in &changed {
    assert_ne!(fingerprint(other), fingerprint(&web), "a changed {what}");
    let rules = [rule(1, &web, false)];
    assert!(
      answered_by(&rules, &start("start_service", &[other])).is_none(),
      "a changed {what} asks again"
    );
  }
  // What a start runs is its program, arguments, variables and seed: its readiness or needs do not ask again.
  let ready = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(90).expect("in range"));
  let waits_longer = Service::new(web.name().clone(), web.program().clone(), ready)
    .with_arguments(web.arguments().to_vec())
    .with_variables(web.variables().clone());
  assert_eq!(fingerprint(&waits_longer), fingerprint(&web));
}

#[test]
fn service_rules_name_what_they_cover_and_never_a_value() {
  let web = service("web").with_variables(variables(&[
    ("MODE", ServiceValue::Plain(TOKEN.to_owned())),
    (
      "API_TOKEN",
      ServiceValue::Secret(SecretName::new("SERVICE_TOKEN").expect("a name")),
    ),
  ]));
  let approval = ServiceApproval::of(&web);
  assert_eq!(approval.name, "web");
  assert_eq!(approval.program, "node");
  assert_eq!(approval.arguments, ["server.js", "--port", "{{port}}"]);
  assert_eq!(approval.variables, ["API_TOKEN", "MODE"], "the names, sorted");
  assert_eq!(approval.command_line(), "node server.js --port {{port}}");
  assert_eq!(approval.short_fingerprint(), &approval.fingerprint[..7]);
  let target = start_target(std::slice::from_ref(&approval));
  assert!(
    !target.contains(TOKEN),
    "a plain value never reaches the request: {target}"
  );
  assert!(
    !target.contains("SERVICE_TOKEN"),
    "nor which secret a variable reads: {target}"
  );
  let scope = ProjectScope::Service(approval.clone());
  assert_eq!((scope.server(), scope.tool()), (SERVER, "start_service"));
  assert_eq!(scope.service(), Some(&approval));
  assert_eq!((scope.script(), scope.rule_scope()), (None, None));
}

#[test]
fn service_rules_answer_a_start_only_when_every_service_has_one() {
  let (web, api) = (service("web"), service("api"));
  let both = [rule(1, &web, false), rule(2, &api, false)];
  assert_eq!(
    answered_by(&both, &start("start_service", &[&api, &web])),
    Some(vec![2, 1]),
    "one rule per service, in the start's order"
  );
  assert_eq!(
    answered_by(&both, &start("start_stack", &[&web])),
    Some(vec![1]),
    "a stack's start runs the same services"
  );
  assert!(
    answered_by(&both[..1], &start("start_service", &[&api, &web])).is_none(),
    "a need without a rule asks"
  );
  let revoked = [rule(1, &web, false), rule(2, &api, true)];
  assert!(answered_by(&revoked, &start("start_service", &[&api, &web])).is_none());
  let unread = ApprovalSubject::McpTool {
    server: SERVER.to_owned(),
    tool: "start_service".to_owned(),
    reach: ToolReach::Services { services: Vec::new() },
  };
  assert!(ProjectScope::of(&unread).is_empty(), "a start that names no service");
  assert!(answered_by(&both, &unread).is_none());
  let elsewhere = ApprovalSubject::McpTool {
    server: "pencil".to_owned(),
    tool: "start_service".to_owned(),
    reach: ToolReach::Services {
      services: vec![ServiceApproval::of(&web)],
    },
  };
  assert!(ProjectScope::of(&elsewhere).is_empty(), "another server's tool");
}

#[test]
fn service_rules_are_read_from_the_request_alone() {
  let (web, api) = (service("web"), service("api"));
  let approvals = vec![ServiceApproval::of(&api), ServiceApproval::of(&web)];
  let target = start_target(&approvals);
  assert_eq!(started_by(&target), Some(approvals.clone()));
  let request = |action: &str, target: &str| PendingToolApproval {
    id: "invocation:1".to_owned(),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Implementer".to_owned(),
    command: false,
    scope: None,
    server: SERVER.to_owned(),
    action: action.to_owned(),
    target: Some(target.to_owned()),
    requested_at: Timestamp::now(),
    native: None,
  };
  let scopes: Vec<ProjectScope> = approvals.iter().cloned().map(ProjectScope::Service).collect();
  assert_eq!(ProjectScope::of_request(&request("start_service", &target)), scopes);
  assert_eq!(ProjectScope::of_request(&request("start_stack", &target)), scopes);
  assert!(ProjectScope::of_request(&request("start_service", "web")).is_empty());
  let forged = target.replace(&approvals[0].fingerprint, "not-a-fingerprint");
  assert_eq!(started_by(&forged), None, "a fingerprint that is not a SHA-256");
  assert!(ProjectScope::of_request(&request("start_qa_app", &target)).is_empty());
}

#[test]
fn service_rules_of_a_run_cover_the_tool() {
  let subject = start("start_service", &[&service("web")]);
  assert_eq!(
    RuleScope::similar(&subject),
    Ok(RuleScope::SimilarMcpTool {
      server: SERVER.to_owned(),
      tool: "start_service".to_owned(),
      folder: None,
      check: None,
    })
  );
}
