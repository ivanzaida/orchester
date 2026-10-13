use orchester_core::{
  id::{ProjectId, RunId},
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_workspace::{tasks::integration_jobs::ProjectJobs, workspace::OpenProject};

#[test]
fn local_integration_work_status_names_the_task_and_releases_the_checkout_queue() {
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("TEST").expect("project key"),
    Name::new("Test project").expect("name"),
  );
  let open = OpenProject {
    project,
    repo_root: env!("CARGO_MANIFEST_DIR").into(),
  };
  let task = "TEST-24".parse().expect("task key");
  let jobs = ProjectJobs::default();
  let project_jobs = jobs.of(open.project.id());
  let run = project_jobs.of(RunId::generate());
  let other = project_jobs.of(RunId::generate());
  let working = run.local_work(&open, &task).expect("checkout queue available");
  let busy = jobs.busy();
  assert_eq!(busy.len(), 1);
  assert_eq!(&*busy[0].project, "Test project");
  assert_eq!(busy[0].what.key, "localIntegration.running");
  assert_eq!(busy[0].what.args, [("task", "TEST-24".to_owned())]);
  assert!(busy[0].merge, "window close tracks local checkout work");
  assert!(run.state().busy());
  assert!(
    other.local_work(&open, &task).is_none(),
    "another run cannot take the checkout"
  );
  assert_eq!(jobs.busy(), busy, "refusal retains the active task's status");
  drop(working);
  assert!(jobs.busy().is_empty());
  assert!(!run.state().busy());
  let next = other.local_work(&open, &task).expect("checkout queue released");
  assert!(other.state().busy());
  drop(next);
  assert!(jobs.busy().is_empty());
}
