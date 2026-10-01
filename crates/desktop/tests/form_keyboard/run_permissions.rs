//! Changing a run's permissions from the task page (02.J), headlessly with the real runner and
//! `orchester-fake-codex`: the popover changes the permissions for the run, Apply now shows the
//! change pending until the agent restarts and then applied, Also save to the profile adds the
//! change to the profile's draft, Keep for next invocation instead cancels the restart, and
//! Escape or Cancel close the popover without changes. On `orchester-fake-claude` the popover says
//! what Claude Code runs besides the allowed commands.

use std::{fs, path::Path};

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  permission::RepositoryAccess,
  resource::{AgentProfile, ResourceKind},
  store::{ActivityStore, ResourceStore},
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, run_finished, seed_agent,
  },
};

fn desktop_running(instructions: &'static str) -> Desktop {
  running_on(AgentProvider::CodexCli, instructions)
}

/// The task page while the Builder on `provider`'s fake CLI runs its first command.
fn running_on(provider: AgentProvider, instructions: &'static str) -> Desktop {
  let (runner, fake) = programs();
  let fake = match provider {
    AgentProvider::CodexCli => fake,
    AgentProvider::ClaudeCodeCli => fake_claude(&fake),
  };
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, provider, |builder| {
      vec![agent_step(builder, "implement", "Implement", instructions, &[])]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the agent's command", |desktop| {
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
fn run_permissions_popover_fits_the_workspace_when_the_window_is_resized() {
  let mut desktop = desktop_running("Wait for release. Draft first. Then keep working.");
  desktop.click("task-permissions-view");
  desktop.wait_for("task-permissions-popover");
  // Any command in the worktree on Codex: what needs more than the sandbox asks a person.
  assert!(shows(
    &mut desktop,
    "task-permissions-popover",
    "Beyond the sandbox, commands ask you first."
  ));
  // App Server still applies the user's own Codex rules, never the repository's; the popover says so too.
  assert!(shows(
    &mut desktop,
    "task-permissions-popover",
    "Your own Codex rules still apply: a command they allow runs outside the sandbox without asking you. \
     The repository’s .codex settings are ignored."
  ));
  for (width, height) in [(1440, 960), (960, 700), (1440, 960)] {
    desktop.tree.resize(width, height);
    desktop.pass();
    let viewport = desktop.bounds("task-permissions-scroll");
    let status = desktop.bounds("status-bar");
    assert!(viewport.y >= orchester_desktop::theme::TITLE_BAR_HEIGHT);
    assert!(viewport.y + viewport.height <= status.y, "{viewport:?} {status:?}");
    if height == 960 {
      let cancel = desktop.bounds("task-permissions-cancel");
      assert!(cancel.y + cancel.height <= status.y, "the footer stays visible");
    }
  }
  desktop.click("task-permissions-cancel");
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
  desktop.wait_until("the pending change", |desktop| {
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
  desktop.wait_until("the applied change", |desktop| has(desktop, "task-permissions-applied"));
  assert!(shows(
    &mut desktop,
    "task-permissions-applied",
    "New permissions active since"
  ));
  assert!(shows(&mut desktop, "task-permissions-applied", "(invocation 2)"));
  assert!(shows(&mut desktop, "task-permissions", "Edit files"));
  fs::remove_file(orchester(desktop.repo()).join("fake-hold")).expect("let go");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}

#[test]
fn run_permissions_popover_closes_without_changes_and_keeps_a_change_for_the_next_invocation() {
  let mut desktop = desktop_running("Wait for release.");
  desktop.click("task-permissions-view");
  desktop.wait_for("task-permissions-popover");
  // The MCP servers the agent gets are shown, read-only: this profile chooses none.
  let mcp = desktop.tree.get_element_by_id("task-permissions-mcp");
  assert!(mcp.is_some_and(|mcp| crate::mcp_servers::contains_text(mcp, "None")));
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
  desktop.wait_until("the pending change", |desktop| has(desktop, "task-permissions-pending"));
  desktop.click("task-permissions-keep");
  desktop.wait_until("the kept change", |desktop| {
    !pending(desktop.repo()) && !has(desktop, "task-permissions-pending")
  });
  assert!(changed(desktop.repo()));
  assert!(
    !has(&mut desktop, "task-permissions-saved"),
    "nothing was saved to the profile"
  );
  fs::write(orchester(desktop.repo()).join("fake-release"), "go").expect("release");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  let run = latest_run(desktop.repo()).expect("run");
  let db = ProjectDb::open(desktop.repo()).expect("open");
  assert_eq!(
    db.invocations(run.id()).expect("invocations").len(),
    1,
    "the agent was not restarted"
  );
}

#[test]
fn run_permissions_popover_on_claude_says_which_commands_run_unlisted() {
  let mut desktop = running_on(AgentProvider::ClaudeCodeCli, "Wait for release.");
  desktop.click("task-permissions-view");
  desktop.wait_for("task-permissions-popover");
  assert!(shows(
    &mut desktop,
    "task-permissions-popover",
    "Claude Code also runs known-safe read-only commands, such as ls, cat or git log, without listing them. Other \
     commands ask you first."
  ));
  assert!(!shows(
    &mut desktop,
    "task-permissions-popover",
    "also allows any arguments"
  ));
  desktop.click("task-permissions-add-command");
  desktop.wait_for("task-permissions-new-command");
  desktop.type_text("rm -f build/x");
  desktop.enter();
  desktop.wait_for("task-permissions-command-1");
  assert!(shows(
    &mut desktop,
    "task-permissions-popover",
    "rm -f build/x also allows any arguments after it, including ones that change files."
  ));
  desktop.click("task-permissions-cancel");
  fs::write(orchester(desktop.repo()).join("fake-release"), "go").expect("release");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
}
