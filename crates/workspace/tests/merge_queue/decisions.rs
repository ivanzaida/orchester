//! A person's decisions on the queue and what holds them to the work they saw: only a person approves a merge, an
//! approval is void once the run worked again, *I've resolved it* puts a run back whether or not its conflicts went to
//! an Integrate step first, and the merge itself refuses a base that moved after the queue's last look at it.

use std::fs;

use orchester_core::{
  integration::{DeliveredAs, Delivering, MergeError},
  merge_queue::{Left, QueueState},
  revision::Revision,
  run::Answerer,
};
use orchester_ipc::Command as RunnerCommand;
use orchester_workspace::{
  merge_queue::{self, QueueRefusal},
  messages::Message,
  tasks::integration::{IntegrationProblem, merge_in_as},
};

use super::harness::{Repo, at, git};

#[test]
fn merge_queue_approval_is_a_persons_and_is_void_once_the_integrate_step_changed_the_work() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let mut run = repo.succeeded(
    (&task, "approve"),
    "orchester/orc-1-run-1",
    ("shared.txt", "the run's line\n"),
    100,
  );
  repo.drive();
  assert_eq!(repo.state(&run), QueueState::WaitingForApproval);
  assert_eq!(
    merge_queue::approve(repo.root(), run.id(), at(1_000), Answerer::Automation),
    Err(QueueRefusal::PersonOnly),
    "an MCP client pressing the button does not approve"
  );
  assert_eq!(repo.state(&run), QueueState::WaitingForApproval);

  merge_queue::approve(repo.root(), run.id(), at(1_001), Answerer::Person).expect("approved");
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  let commands = repo.drive();
  assert!(
    matches!(commands.as_slice(), [RunnerCommand::ReturnRun(_)]),
    "{commands:?}"
  );
  merge_queue::integrate_sent(repo.root(), run.id(), Ok(())).expect("told");
  repo.integrated(&mut run, ("shared.txt", "both lines\n"), 2_000);
  repo.drive();

  assert_eq!(
    repo.state(&run),
    QueueState::WaitingForApproval,
    "the approval was for the work before the Integrate step changed it"
  );
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  assert!(!repo.main_holds(&tip), "nothing merged without a new approval");
  merge_queue::approve(repo.root(), run.id(), at(3_000), Answerer::Person).expect("approved again");
  repo.drive();
  assert!(
    matches!(repo.state(&run), QueueState::Merged(_)),
    "{:?}",
    repo.state(&run)
  );
}

#[test]
fn merge_queue_requeues_a_run_resolved_by_hand_when_its_pipeline_has_no_integrate_step() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(
    (&task, "unfit"),
    "orchester/orc-1-run-1",
    ("shared.txt", "the run's line\n"),
    100,
  );
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  assert_eq!(repo.drive(), Vec::new(), "no Integrate step to send it to");
  assert_eq!(
    repo.state(&run),
    QueueState::Left(Left::Conflicts {
      files: vec!["shared.txt".to_owned()]
    })
  );

  let files = merge_queue::resolve_myself(repo.root(), &repo.programs(), run.id()).expect("merged in");
  assert_eq!(files, ["shared.txt"]);
  fs::write(repo.worktree(&run).join("shared.txt"), "both lines\n").expect("resolve");
  let command = merge_queue::resolved(repo.root(), &repo.programs(), run.id()).expect("resolved");
  assert_eq!(command, None, "no step to send the work back to");
  merge_queue::requeue(repo.root(), run.id(), Ok(()), at(1_000)).expect("requeued");
  assert_eq!(repo.state(&run), QueueState::Queued);
  repo.drive();
  assert!(
    matches!(repo.state(&run), QueueState::Merged(_)),
    "{:?}",
    repo.state(&run)
  );
  assert_eq!(
    fs::read_to_string(repo.root().join("shared.txt")).expect("read"),
    "both lines\n"
  );
}

#[test]
fn merge_queue_resolved_by_hand_after_the_integrate_step_could_not_be_told_goes_back_to_it() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(
    (&task, "auto"),
    "orchester/orc-1-run-1",
    ("shared.txt", "the run's line\n"),
    100,
  );
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  repo.drive();
  let no_runner = Message::new("errors.storage");
  merge_queue::integrate_sent(repo.root(), run.id(), Err(no_runner.clone())).expect("told");

  merge_queue::resolve_myself(repo.root(), &repo.programs(), run.id()).expect("merged in");
  fs::write(repo.worktree(&run).join("shared.txt"), "both lines\n").expect("resolve");
  let command = merge_queue::resolved(repo.root(), &repo.programs(), run.id()).expect("resolved");
  let Some(RunnerCommand::ReturnRun(returned)) = command else {
    panic!("{command:?}");
  };
  assert_eq!(returned.step.as_str(), "implement", "the integrator profile's step");
  assert_eq!(
    merge_queue::requeue(repo.root(), run.id(), Err(no_runner.clone()), at(1_000)),
    Err(QueueRefusal::Resolve(no_runner)),
    "the runner's words, and the run stays out of the queue"
  );
  assert!(matches!(repo.state(&run), QueueState::Left(Left::Conflicts { .. })));
  let again = merge_queue::resolved(repo.root(), &repo.programs(), run.id()).expect("pressed again");
  assert!(matches!(again, Some(RunnerCommand::ReturnRun(_))), "{again:?}");
  merge_queue::requeue(repo.root(), run.id(), Ok(()), at(1_001)).expect("requeued");
  assert_eq!(repo.state(&run), QueueState::Queued);
}

#[test]
fn merge_queue_merge_refuses_a_base_that_moved_after_the_queue_read_it() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let checked = Revision::new(&git(repo.root(), &["rev-parse", "main"])).expect("revision");
  let tip = Revision::new(&git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"])).expect("revision");
  // Another merge lands between the queue's look at main and its merge.
  Repo::commit_on_main(repo.root(), ("other.txt", "landed meanwhile\n"));
  let delivering = Delivering {
    as_delivered: DeliveredAs::MergeCommit,
    base: &checked,
  };
  let merged = merge_in_as(repo.root(), &repo.programs(), (run.id(), Some(&tip), Some(delivering)));
  assert!(
    matches!(merged, Err(IntegrationProblem::Merge(MergeError::BaseMoved { .. }))),
    "{merged:?}"
  );
  assert!(!repo.main_holds(tip.as_str()), "nothing reached main");
}
