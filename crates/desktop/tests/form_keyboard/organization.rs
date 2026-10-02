use std::path::Path;

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{
    TaskDetails,
    organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{harness::Desktop, task_run::set_text};

pub(crate) fn choose(desktop: &mut Desktop, id: &str, label: &str) {
  let mut select = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .expect("select");
  let index = select
    .labels()
    .iter()
    .position(|option| option.as_ref() == label)
    .expect("option");
  select.commit(index);
  desktop.pass();
}
pub(crate) fn tasks(repo: &Path) -> Vec<orchester_core::task::Task> {
  ProjectDb::open(repo).expect("open").tasks().expect("tasks")
}

#[test]
fn organization_new_form_saves_status_priority_informational_reason_and_inline_tag() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-title");
  set_text(&mut desktop, "task-title", "Organized task");
  choose(&mut desktop, "task-workflow-field", "Todo");
  choose(&mut desktop, "task-priority-field", "High");
  desktop.tab_to("task-blocked-field", 40);
  desktop.enter();
  desktop.wait_for("task-blocked-reason");
  set_text(&mut desktop, "task-blocked-reason", "Waiting for a decision");
  desktop.press("task-tags-picker");
  desktop.wait_for("tag-search");
  set_text(&mut desktop, "tag-search", "Desktop");
  desktop.tab_to("tag-create-inline", 40);
  desktop.enter();
  desktop.wait_until("inline tag creation", |desktop| {
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len()
      == 1
  });
  desktop.wait_until("inline tag selected in the form", |desktop| {
    crate::github::shows(desktop, "task-tag-chips", "Desktop")
  });
  desktop.key("Escape", "Escape");
  desktop.tab_to("save-draft", 40);
  desktop.enter();
  desktop.wait_until("organization saved", |desktop| {
    tasks(&desktop.repo)
      .first()
      .is_some_and(|task| task.organization().tags().len() == 1)
  });
  let task = tasks(&desktop.repo).remove(0);
  assert_eq!(task.organization().status(), WorkflowStatus::Todo);
  assert_eq!(task.organization().priority(), Priority::High);
  assert_eq!(task.organization().blocked_reason(), Some("Waiting for a decision"));
  ProjectDb::open(&desktop.repo)
    .expect("open")
    .check_task_readiness(task.id())
    .expect("informational flag permits run");
}

#[test]
fn organization_bulk_status_priority_and_filters() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("First").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    db.create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("tasks");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  let ids: Vec<_> = tasks(&desktop.repo).iter().map(|task| task.id()).collect();
  for task in tasks(&desktop.repo) {
    desktop.press(&format!("task-select-{}", task.key()));
  }
  desktop.press("bulk-status");
  desktop.wait_for("bulk-status-InReview");
  desktop.press("bulk-status-InReview");
  desktop.wait_until("bulk status", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().status() == WorkflowStatus::InReview)
  });
  desktop.wait_for(&format!("bulk-outcome-{}", ids[0]));
  desktop.press("bulk-priority");
  desktop.wait_for("bulk-priority-Urgent");
  desktop.press("bulk-priority-Urgent");
  desktop.wait_until("bulk priority", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().priority() == Priority::Urgent)
  });
  desktop.wait_until("priority visible after bulk save", |desktop| {
    crate::github::shows(desktop, "task-row-KON-1", "Urgent")
  });
  desktop.press("filter-status");
  desktop.wait_for("filter-status-Todo");
  desktop.press("filter-status-Todo");
  assert!(desktop.tree.get_element_by_id("task-row-KON-1").is_none());
  desktop.press("filters-clear");
  desktop.wait_for("task-row-KON-1");
}

#[test]
fn organization_tag_delete_confirmation_escape_preserves_assignments() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("Tagged").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    let task = db
      .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
      .expect("task")[0]
      .id();
    let tag = db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
    db.set_task_organization(
      task,
      &TaskOrganization::default().with_tags(vec![tag.id()]),
      ChangeActor::Person,
      Timestamp::now(),
    )
    .expect("tagging");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  let tag = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  let delete = format!("project-tag-delete-{tag}");
  desktop.wait_for(&delete);
  desktop.press(&delete);
  desktop.wait_for("tag-editor-confirm");
  desktop.key("Escape", "Escape");
  assert_eq!(tasks(&desktop.repo)[0].organization().tags(), &[tag]);
  desktop.press(&delete);
  desktop.wait_for("tag-editor-confirm");
  desktop.tab_to("tag-editor-confirm", 8);
  desktop.enter();
  desktop.wait_until("tag deletion removes assignments", |desktop| {
    tasks(&desktop.repo)[0].organization().tags().is_empty()
  });
  assert!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .is_empty()
  );
}

#[test]
fn organization_cancel_active_run_confirms_and_preserves_branch() {
  use crate::task_run::{create_and_run, init_repository, latest_run, programs, seed};
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("active run", |desktop| {
    latest_run(&desktop.repo).is_some_and(|run| !run.status().is_terminal())
  });
  let run = latest_run(&desktop.repo).expect("run");
  desktop.wait_for("task-header-status");
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-confirmation");
  assert_eq!(
    tasks(&desktop.repo)[0].organization().status(),
    WorkflowStatus::InProgress
  );
  desktop.key("Escape", "Escape");
  assert!(desktop.tree.get_element_by_id("task-cancel-confirmation").is_none());
  assert!(!latest_run(&desktop.repo).expect("run").status().is_terminal());
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-stop");
  desktop.tab_to("task-cancel-stop", 5);
  desktop.enter();
  desktop.wait_until("task canceled after stop", |desktop| {
    tasks(&desktop.repo)[0].organization().status() == WorkflowStatus::Canceled
  });
  let stopped = latest_run(&desktop.repo).expect("run");
  assert!(stopped.status().is_terminal());
  assert_eq!(stopped.branch(), run.branch());
  assert!(
    desktop
      .repo
      .join(".orchester/worktrees")
      .join(run.id().to_string())
      .is_dir()
  );
}

#[test]
fn organization_readiness_shows_reason_blocks_run_and_refreshes_after_resolution() {
  use orchester_core::{
    integration::{MergeKind, RunMerge},
    revision::Revision,
    run::BranchName,
    store::{MergeStore, RunStore},
    task::TaskStatus,
  };

  use crate::github::shows;
  let mut desktop = Desktop::with_seeded_project(seed_readiness);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-2");
  desktop.press("task-row-KON-2");
  desktop.wait_for("task-run");
  assert!(shows(&mut desktop, "task-page", "Waiting for integration"));
  assert!(shows(
    &mut desktop,
    "task-dependency-readiness",
    "Run is unavailable while KON-1 is unfinished."
  ));
  desktop.press("task-run");
  assert_eq!(tasks(&desktop.repo)[1].status(), TaskStatus::Draft);
  let mut db = ProjectDb::open(&desktop.repo).expect("open");
  let blocker = tasks(&desktop.repo)[0].id();
  let run = db.runs_of_task(blocker).expect("runs").remove(0).expect("readable run");
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("merge");
  drop(db);
  choose(&mut desktop, "task-header-priority", "Low");
  desktop.wait_until("resolved readiness", |desktop| {
    shows(desktop, "task-dependency-readiness", "Ready to queue")
  });
  assert_eq!(
    tasks(&desktop.repo)[1].organization().blocked_reason(),
    Some("Waiting for integration")
  );
}

#[test]
fn organization_tag_rename_and_merge_preserve_task_assignment() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("Tagged").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    let task = db
      .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
      .expect("task")[0]
      .id();
    let source = db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
    let target = db.create_project_tag(Name::new("UX").expect("name")).expect("tag");
    db.set_task_organization(
      task,
      &TaskOrganization::default().with_tags(vec![source.id(), target.id()]),
      ChangeActor::Person,
      Timestamp::now(),
    )
    .expect("tags");
  });
  let tags = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags");
  let source = tags
    .iter()
    .find(|tag| tag.name().as_str() == "Desktop")
    .expect("source")
    .id();
  let target = tags
    .iter()
    .find(|tag| tag.name().as_str() == "UX")
    .expect("target")
    .id();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  let rename = format!("project-tag-rename-{source}");
  desktop.wait_for(&rename);
  desktop.press(&rename);
  desktop.wait_for("project-tag-name");
  set_text(&mut desktop, "project-tag-name", "Client");
  desktop.tab_to("tag-editor-confirm", 6);
  desktop.enter();
  desktop.wait_until("tag renamed", |desktop| {
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .iter()
      .any(|tag| tag.name().as_str() == "Client")
  });
  desktop.wait_until("rename dialog closed", |desktop| {
    desktop.tree.get_element_by_id("tag-editor-confirm").is_none()
  });
  desktop.press(&format!("project-tag-merge-{source}"));
  desktop.wait_for("tag-merge-target");
  choose(&mut desktop, "tag-merge-target", "UX · 1 task");
  desktop.tab_to("tag-editor-confirm", 6);
  desktop.enter();
  desktop.wait_until("merge deduplicates task assignment", |desktop| {
    tasks(&desktop.repo)[0].organization().tags() == [target]
  });
  assert_eq!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len(),
    1
  );
}

#[test]
fn organization_bulk_add_and_remove_tags_updates_every_selected_task() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("Bulk tagged").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    db.create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("tasks");
    db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
  });
  let tag = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  let ids: Vec<_> = tasks(&desktop.repo).iter().map(|task| task.id()).collect();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  for task in tasks(&desktop.repo) {
    desktop.press(&format!("task-select-{}", task.key()));
  }
  desktop.press("bulk-add-tags");
  // The panel shows the tag choices in place (`HI91q`), without a picker button to open.
  desktop.wait_for(&format!("picker-tag-{tag}"));
  assert!(desktop.tree.get_element_by_id("task-tags-picker").is_none());
  desktop.press(&format!("picker-tag-{tag}"));
  desktop.click("bulk-tags-apply");
  desktop.wait_until("bulk tag addition", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().tags() == [tag])
  });
  desktop.wait_for(&format!("bulk-outcome-{}", ids[0]));
  desktop.wait_until("tag chips refreshed", |desktop| {
    crate::github::shows(desktop, "task-row-KON-1", "Desktop")
  });
  desktop.press("bulk-remove-tags");
  desktop.wait_for(&format!("bulk-remove-tag-{tag}"));
  desktop.press(&format!("bulk-remove-tag-{tag}"));
  assert!(
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().tags() == [tag]),
    "selecting removal must not mutate tasks"
  );
  desktop.press("bulk-tags-remove-apply");
  desktop.wait_until("bulk tag removal", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().tags().is_empty())
  });
}

fn seed_readiness(repo: &Path) {
  use orchester_core::{
    resource::ResourceKind,
    store::{ResourceStore, RunStore},
    task::{
      TaskLimits,
      organization::{BlockingTask, DependencyGate},
    },
    tokens::TokenBudget,
  };

  use crate::unreadable_run::{at, delivery, started};
  let mut db = ProjectDb::open(repo).expect("open");
  let snapshot = delivery(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let details = TaskDetails {
    title: Name::new("Linked task").expect("title"),
    brief: "Implement.".into(),
    acceptance_criteria: vec!["Complete.".into()],
    pipeline: Some(pipeline),
    limits: Some(
      TaskLimits::new(
        TokenBudget::new(10000).expect("budget"),
        std::time::Duration::from_secs(1800),
      )
      .expect("limits"),
    ),
  };
  let tasks = db
    .create_tasks(IdempotencyKey::generate(), vec![details.clone(), details], at(10))
    .expect("tasks");
  let mut run = started(&mut db, (tasks[0].id(), &snapshot), 1, 11);
  let events = run.cancel(at(12)).expect("stop fixture run");
  db.append_events(&run, &events).expect("stop");
  let organization = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    Some("Waiting for integration".into()),
    vec![BlockingTask {
      task: tasks[0].id(),
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(tasks[1].id(), &organization, ChangeActor::Person, at(13))
    .expect("link");
}

#[test]
fn organization_add_link_skips_empty_and_cyclic_choices() {
  use orchester_core::task::organization::{BlockingTask, DependencyGate};
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("Dependency").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    let tasks = db
      .create_tasks(
        IdempotencyKey::generate(),
        vec![details.clone(), details.clone(), details],
        Timestamp::now(),
      )
      .expect("tasks");
    let linked = TaskOrganization::new(
      WorkflowStatus::Todo,
      Priority::None,
      None,
      vec![BlockingTask {
        task: tasks[0].id(),
        gate: DependencyGate::Merge,
      }],
      vec![],
    )
    .expect("organization");
    db.set_task_organization(tasks[1].id(), &linked, ChangeActor::Person, Timestamp::now())
      .expect("link");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-blocked-field");
  desktop.tab_to("task-blocked-field", 40);
  desktop.enter();
  desktop.wait_for("task-blocked-reason");
  set_text(&mut desktop, "task-blocked-reason", "Needs a dependency");
  desktop.wait_for("task-blocking-picker");
  assert!(
    !desktop
      .stops(40, false)
      .contains(&Some("task-add-blocking-link".into()))
  );
  choose(&mut desktop, "task-blocking-picker", "KON-2 · Dependency · Todo");
  assert!(
    !desktop
      .stops(40, false)
      .contains(&Some("task-add-blocking-link".into()))
  );
  desktop.press("task-add-blocking-link");
  assert!(desktop.tree.get_element_by_id("task-add-blocking-link").is_some());
  choose(&mut desktop, "task-blocking-picker", "KON-3 · Dependency · Backlog");
  assert!(
    desktop
      .stops(40, false)
      .contains(&Some("task-add-blocking-link".into()))
  );
  desktop.tab_to("task-add-blocking-link", 40);
  desktop.enter();
  desktop.tab_to("save-draft", 40);
  desktop.enter();
  desktop.wait_until("valid dependency saved", |desktop| {
    tasks(&desktop.repo)[0].organization().dependencies().len() == 1
  });
  let stored = tasks(&desktop.repo);
  assert_eq!(stored[0].organization().dependencies()[0].task, stored[2].id());
}
