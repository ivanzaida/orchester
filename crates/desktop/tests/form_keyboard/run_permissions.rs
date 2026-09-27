//! Changing a run's permissions from the task page (02.J), headlessly with the real runner and
//! `orchester-fake-codex`: the popover changes the permissions for the run, Apply now shows the
//! change pending until the agent restarts and then applied, Also save to the profile adds the
//! change to the profile's draft, Keep for next invocation instead cancels the restart, and
//! Escape or Cancel close the popover without changes. Windows only, like `task_run`.
#![cfg(windows)]

use std::{fs, path::Path};

use orchester_core::{
  connection::stream::AgentEvent,
  permission::RepositoryAccess,
  resource::{AgentProfile, ResourceKind},
  store::{ActivityStore, ResourceStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_steps, wait_long,
  },
};

fn desktop_running(instructions: &'static str) -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![agent_step(builder, "implement", "Implement", instructions, &[])]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  wait_long(&mut desktop, "the agent's command", |desktop| {
    command_started(desktop.repo()) && has(desktop, "task-permissions-view")
  });
  desktop
}

/// Whether the fake CLI's `pnpm test` has started.
fn command_started(repo: &Path) -> bool {
  let Some(run) = latest_run(repo) else {
    return false;
  };
  ProjectDb::open(repo)
    .expect("open")
    .activity(run.id(), 0)
    .expect("activity")
    .iter()
    .any(|entry| matches!(&entry.event, AgentEvent::CommandStarted { command, .. } if command == "pnpm test"))
}

fn orchester(repo: &Path) -> std::path::PathBuf {
  repo.join(".orchester")
}

fn changed(repo: &Path) -> bool {
  let run = latest_run(repo).expect("run");
  let profile = run
    .profile_of(&run.snapshot().steps()[0].key().clone())
    .expect("agent step");
  run.changed_permissions(profile).is_some()
}

fn pending(repo: &Path) -> bool {
  let run = latest_run(repo).expect("run");
  let profile = run
    .profile_of(&run.snapshot().steps()[0].key().clone())
    .expect("agent step");
  run
    .changed_permissions(profile)
    .is_some_and(orchester_core::run::RunPermissions::is_pending)
}

#[test]
fn run_permissions_apply_now_is_pending_then_applied_and_saved_to_the_profile_draft() {
  let mut desktop = desktop_running("Wait for release. Draft first. Then keep working.");
  assert!(shows(&mut desktop, "task-permissions", "change ↗"));
  desktop.click("task-permissions-view");
  desktop.wait_for("task-permissions-popover");
  assert!(shows(
    &mut desktop,
    "task-permissions-popover",
    "Both change Run 1 only."
  ));
  desktop.click("task-permissions-edit");
  desktop.click("task-permissions-save-to-profile");
  assert!(shows(&mut desktop, "task-permissions-save-to-profile", "draft v2"));
  desktop.click("task-permissions-apply-now");
  wait_long(&mut desktop, "the pending change", |desktop| {
    !has(desktop, "task-permissions-popover") && has(desktop, "task-permissions-pending")
  });
  assert!(shows(&mut desktop, "task-permissions", "Applying at next checkpoint…"));
  assert!(shows(&mut desktop, "task-permissions-saved", "draft v2"));
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let builder = db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let draft = db.draft::<AgentProfile>(builder).expect("read").expect("a draft");
  assert_eq!(draft.body().permissions().repository(), RepositoryAccess::Write);
  drop(db);

  fs::write(orchester(desktop.repo()).join("fake-hold"), "hold").expect("hold");
  fs::write(orchester(desktop.repo()).join("fake-release"), "go").expect("release");
  wait_long(&mut desktop, "the applied change", |desktop| {
    has(desktop, "task-permissions-applied")
  });
  assert!(shows(
    &mut desktop,
    "task-permissions-applied",
    "New permissions active since"
  ));
  assert!(shows(&mut desktop, "task-permissions-applied", "(invocation 2)"));
  assert!(shows(&mut desktop, "task-permissions", "Edit files"));
  fs::remove_file(orchester(desktop.repo()).join("fake-hold")).expect("let go");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
}

#[test]
fn run_permissions_popover_closes_without_changes_and_keeps_a_change_for_the_next_invocation() {
  let mut desktop = desktop_running("Wait for release.");
  desktop.click("task-permissions-view");
  desktop.wait_for("task-permissions-popover");
  // The popover is a form: Tab reaches its controls.
  desktop.tab_to("task-permissions-save-to-profile", 12);
  // Nothing changed yet: neither way to apply does anything.
  desktop.click("task-permissions-apply-next");
  desktop.click("task-permissions-apply-now");
  desktop.key("Escape", "Escape");
  assert!(!has(&mut desktop, "task-permissions-popover"));
  desktop.click("task-permissions-view");
  desktop.click("task-permissions-edit");
  desktop.click("task-permissions-cancel");
  assert!(!has(&mut desktop, "task-permissions-popover"));
  assert!(!changed(desktop.repo()), "Escape and Cancel change nothing");

  desktop.click("task-permissions-view");
  desktop.click("task-permissions-edit");
  desktop.click("task-permissions-apply-now");
  wait_long(&mut desktop, "the pending change", |desktop| {
    has(desktop, "task-permissions-pending")
  });
  desktop.click("task-permissions-keep");
  wait_long(&mut desktop, "the kept change", |desktop| {
    !pending(desktop.repo()) && !has(desktop, "task-permissions-pending")
  });
  assert!(changed(desktop.repo()));
  assert!(
    !has(&mut desktop, "task-permissions-saved"),
    "nothing was saved to the profile"
  );
  fs::write(orchester(desktop.repo()).join("fake-release"), "go").expect("release");
  wait_long(&mut desktop, "the run to finish", |desktop| {
    run_finished(desktop.repo())
  });
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert_eq!(
    db.invocations(run.id()).expect("invocations").len(),
    1,
    "the agent was not restarted"
  );
}
