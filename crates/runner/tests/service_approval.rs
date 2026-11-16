//! Approving a service start end to end against `orchester-fake-codex`, with `orchester-test-command` as the services
//! (`support::services`; `docs/architecture/agent-testing.md`, "Service › approval"): a start asks a person once for
//! the service and everything it needs, its request naming each with its program, arguments and variable names, never
//! their values. *Approve for every run* keeps one rule of the project per service; a later run's start of the service,
//! or of a stack of the same services, is then answered by the rules without asking and without auto mode's reviewer.
//! A service published again with another argument asks again, and a start never runs a definition its approval did
//! not name. Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test service_approval
//! service_approval`.

mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  resource::{Draft, ResourceBody, VariableName},
  run::{Answerer, ApprovalAnswer, DecidedBy, RunStatus},
  service::{ReadyCheck, Service, ServiceName, ServiceValue, ServiceVariables, started_by},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc::UnboundedReceiver;

use self::support::{
  StepSpec, pending_approval,
  qa::calls,
  queue_another_task, scheduler,
  services::{answered, publish, publish_stack, service, service_project},
  wait_for,
};

/// A plain value of a variable, which no request or rule may show.
const PLAIN_VALUE: &str = "plain-value-9c2e";

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The service `name` that needs `needs`, says each of `lines` and listens on its port, with the variable `MODE` set to
/// [`PLAIN_VALUE`]: other lines are another argument.
fn listening(name: &str, needs: &[&str], lines: &[&str]) -> Service {
  let mut arguments: Vec<String> = lines
    .iter()
    .flat_map(|line| ["say".to_owned(), (*line).to_owned()])
    .collect();
  arguments.extend(["listen".to_owned(), "{{port}}".to_owned()]);
  let needs = needs
    .iter()
    .map(|need| ServiceName::new(need).expect("a name"))
    .collect();
  let variables = ServiceVariables::new(BTreeMap::from([(
    VariableName::new("MODE").expect("a name"),
    ServiceValue::Plain(PLAIN_VALUE.to_owned()),
  )]))
  .expect("variables");
  service(name, &arguments, ReadyCheck::PortOpen, 20)
    .with_needs(needs)
    .expect("needs")
    .with_variables(variables)
}

/// Publishes `service` again, as a person does who changes it on the project's Services page.
fn republish(root: &Path, service: &Service) {
  let mut db = ProjectDb::open(root).expect("open");
  let resource = db
    .resources(Service::KIND)
    .expect("services")
    .into_iter()
    .find(|resource| resource.slug().as_str() == service.name().as_str())
    .expect("the service is published");
  db.update_draft::<Service>(resource.id(), |_, latest| {
    Ok(Draft::new(
      service.clone(),
      latest.map(|version| version.number()),
      Timestamp::now(),
    ))
  })
  .expect("a draft");
  db.publish_draft::<Service>(resource.id(), Timestamp::now())
    .expect("published again");
}

/// How `run`'s requests were answered, oldest first.
fn answered_by(root: &Path, run: RunId) -> Vec<Option<DecidedBy>> {
  ProjectDb::open(root)
    .expect("open")
    .decided_tool_approvals(run)
    .expect("decided")
    .into_iter()
    .map(|decided| decided.decided_by)
    .collect()
}

/// Runs `task` to its end, answering its one request with `answer` when given; the run.
async fn run_answering(
  (scheduler, events): (&Scheduler, &mut UnboundedReceiver<Event>),
  root: &Path,
  task: TaskId,
  answer: Option<ApprovalAnswer>,
) -> RunId {
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("run");
  if let Some(answer) = answer {
    let request = pending_approval(root, events, id).await;
    scheduler
      .decide_tool_approval(id, request.id, answer, Answerer::Person)
      .await
      .expect("answered");
  }
  let run = wait_for(root, events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  id
}

/// The answers of `run`'s calls of `tool`.
fn answers(root: &Path, run: RunId, tool: &str) -> Vec<String> {
  calls(root, run)
    .into_iter()
    .filter(|call| call.starts_with(tool))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn service_approval_asks_once_for_a_service_and_its_needs_then_answers_every_run() {
  let project = service_project(implement("Start the service web."), Vec::new());
  let root = project.repo.path();
  publish(
    root,
    &[
      listening("db", &[], &[]),
      listening("api", &["db"], &[]),
      listening("web", &["api"], &[]),
    ],
  );
  let (second, third) = (queue_another_task(root), queue_another_task(root));
  let (scheduler, mut events) = scheduler(root);

  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("first run");
  let request = pending_approval(root, &mut events, first).await;
  assert_eq!(request.action, "start_service");
  let target = request.target.clone().expect("the request names what it starts");
  let named = started_by(&target).expect("the request names the services");
  let names: Vec<&str> = named.iter().map(|service| service.name.as_str()).collect();
  assert_eq!(names, ["db", "api", "web"], "every service of the start, in order");
  assert!(named.iter().all(|service| service.variables == ["MODE"]), "{named:?}");
  assert!(
    !target.contains(PLAIN_VALUE),
    "a variable's value never reaches the request"
  );
  scheduler
    .decide_tool_approval(first, request.id, ApprovalAnswer::SimilarProject, Answerer::Person)
    .await
    .expect("approved for every run");
  let run = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    answered_by(root, first),
    vec![Some(DecidedBy::PersonSimilar)],
    "it asked once"
  );
  assert!(answers(root, first, "start_service")[0].starts_with("start_service ok: "));
  let db = || ProjectDb::open(root).expect("open");
  let rules = db().project_approval_rules().expect("rules");
  let covered: Vec<&str> = rules
    .iter()
    .filter_map(|rule| rule.scope.service())
    .map(|service| service.name.as_str())
    .collect();
  assert_eq!(covered, ["db", "api", "web"], "one rule per service");
  let decided = db().decided_tool_approvals(first).expect("decided");
  assert!(decided[0].for_project, "the request reads as approved for every run");

  // Another run's start is answered by the rules, without asking anybody.
  let later = run_answering((&scheduler, &mut events), root, second, None).await;
  assert_eq!(answered_by(root, later), vec![Some(DecidedBy::SimilarRule)]);
  let decided = db().decided_tool_approvals(later).expect("decided");
  assert_eq!(decided[0].review, None, "auto mode's reviewer never saw it");
  assert!(answers(root, later, "start_service")[0].starts_with("start_service ok: "));
  let rules = db().project_approval_rules().expect("rules");
  assert!(rules.iter().all(|rule| rule.answered == 1), "{rules:?}");

  // A need published with another argument asks again, once, for the whole start.
  republish(root, &listening("db", &[], &["fresh"]));
  let changed = run_answering((&scheduler, &mut events), root, third, Some(ApprovalAnswer::Once)).await;
  assert_eq!(answered_by(root, changed), vec![Some(DecidedBy::Person)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_approval_asks_again_when_a_service_changes_and_runs_only_what_was_approved() {
  let project = service_project(
    implement("Start the stack full. Then start the service web."),
    Vec::new(),
  );
  let root = project.repo.path();
  publish(root, &[listening("db", &[], &[]), listening("web", &["db"], &[])]);
  publish_stack(root, "full", &["web"]);
  let next = queue_another_task(root);
  let (scheduler, mut events) = scheduler(root);

  // The stack's start asks; approved for every run, its rules answer the start of `web` right after it.
  let first = run_answering(
    (&scheduler, &mut events),
    root,
    project.task,
    Some(ApprovalAnswer::SimilarProject),
  )
  .await;
  assert_eq!(
    answered_by(root, first),
    vec![Some(DecidedBy::PersonSimilar), Some(DecidedBy::SimilarRule)]
  );
  assert!(answers(root, first, "start_stack")[0].starts_with("start_stack ok: "));

  // `db` published with another argument: the next run asks again, and while it waits `db` is published once more,
  // so the approved start refuses the definition its approval did not name.
  republish(root, &listening("db", &[], &["--verbose"]));
  let second = scheduler
    .start_run(IdempotencyKey::generate(), next)
    .await
    .expect("run");
  let request = pending_approval(root, &mut events, second).await;
  assert_eq!(request.action, "start_stack");
  republish(root, &listening("db", &[], &["--quiet"]));
  scheduler
    .decide_tool_approval(second, request.id, ApprovalAnswer::Once, Answerer::Person)
    .await
    .expect("approved once");
  let refused = answered(root, &mut events, second, "start_stack").await;
  assert!(
    refused.starts_with("start_stack error: ") && refused.contains("is not the definition this start's approval named"),
    "{refused}"
  );
  // Its start of `web` asks again too, as nothing approved the `db` published last.
  let request = pending_approval(root, &mut events, second).await;
  assert_eq!(request.action, "start_service");
  scheduler
    .decide_tool_approval(second, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  let run = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(answered_by(root, second)[0], Some(DecidedBy::Person));
}
