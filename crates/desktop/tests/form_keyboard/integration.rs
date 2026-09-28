//! Merging a run from Ready for review and removing a finished run's worktree, end to end,
//! headlessly, on the real runner, `orchester-fake-codex` and git: the merge moves `main` and
//! clears the run's worktree and branch, a checkout with uncommitted changes is refused and left
//! alone, and *Remove worktree* names the uncommitted files and removes them only once discarding
//! is agreed to — and not when changes appeared after the dialog listed them; after that, the run's
//! agent can no longer be asked.

use std::{fs, path::Path, process::Command};

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{create_and_run, has, init_repository, latest_run, programs, run_finished, seed},
};

fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(output.status.success(), "git {args:?}");
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

/// A project whose one task ran to the end on the fake Codex, its task page on Ready for review.
fn finished_run() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  desktop.wait_until("the run to finish", |desktop| run_finished(desktop.repo()));
  desktop.wait_until("Ready for review", |desktop| {
    has(desktop, "task-ready") && has(desktop, "task-remove-worktree")
  });
  desktop
}

fn run_worktree(desktop: &Desktop) -> std::path::PathBuf {
  let run = latest_run(desktop.repo()).expect("run");
  desktop
    .repo()
    .join(".orchester")
    .join("worktrees")
    .join(run.id().to_string())
}

#[test]
fn merge_moves_main_to_the_run_and_clears_its_worktree_and_branch() {
  let mut desktop = finished_run();
  let run = latest_run(desktop.repo()).expect("run");
  assert_eq!(run.base_branch().map(|branch| branch.as_str()), Some("main"));
  assert!(shows(&mut desktop, "task-merge", "Merge into main"));
  assert!(shows(&mut desktop, "task-heading", "Ready for review"));
  desktop.click("task-merge");
  desktop.wait_until("the merge", |desktop| {
    holds(desktop, "task-ready-notice", "Merged into main") && holds(desktop, "task-ready-left", "were removed")
  });
  let head = run
    .last_checkpoint()
    .expect("a checkpoint")
    .revision()
    .as_str()
    .to_owned();
  assert_eq!(
    git(desktop.repo(), &["rev-parse", "main"]),
    head,
    "main fast-forwarded to the run"
  );
  assert!(
    fs::read_to_string(desktop.repo().join("README.md"))
      .expect("readme")
      .contains("hello")
  );
  assert!(shows(&mut desktop, "task-heading", "Merged"));
  assert!(shows(
    &mut desktop,
    "task-ready-notice",
    &format!("main moved forward to {}", &head[..7])
  ));
  assert!(shows(
    &mut desktop,
    "task-ready-left",
    "Its worktrees and merged branches were removed."
  ));
  assert!(!has(&mut desktop, "task-merge"), "a merged run is not merged again");
  assert!(
    !has(&mut desktop, "task-ask-agent"),
    "the conversation of a merged run takes no messages"
  );
  assert!(!has(&mut desktop, "task-remove-worktree"), "nothing is left to remove");
  assert!(!run_worktree(&desktop).exists());
  assert!(git(desktop.repo(), &["branch", "--list", run.branch().as_str()]).is_empty());
  merged_conversation_takes_no_messages(&mut desktop);
  // The task list says so too.
  desktop.click("task-breadcrumb");
  desktop.wait_for("task-row-KON-1");
  assert!(shows(&mut desktop, "task-row-KON-1", "Merged"));
}

/// The Conversation tab of the merged run (02.S2 `m3gLZ`): the notice says where the work went, no
/// agent reads messages, so the recipient cannot be chosen and the policy about when agents read
/// is left out, and the permissions, which cannot change any more, have no link.
fn merged_conversation_takes_no_messages(desktop: &mut Desktop) {
  desktop.click("task-tab-conversation");
  desktop.wait_for("task-conversation-panel");
  assert!(shows(desktop, "conversation-status", "Merged"));
  desktop.wait_until("what is left of the run", |desktop| {
    holds(
      desktop,
      "task-notice",
      "Its worktrees and merged branches were removed.",
    )
  });
  assert!(shows(desktop, "task-notice", "Merged into main"));
  assert!(!has(desktop, "task-conversation-policy"));
  assert!(shows(desktop, "task-working-context", "Permissions"));
  assert!(!has(desktop, "task-permissions-view"), "nothing to change");
  assert!(has(desktop, "conversation-input-closed"));
  desktop.press("conversation-recipient");
  let open = desktop
    .tree
    .get_element_by_id_mut("conversation-recipient")
    .and_then(|element| element.as_select())
    .expect("the recipient select")
    .is_open();
  assert!(!open, "the recipient select is disabled");
  desktop.click("task-tab-summary");
  desktop.wait_for("task-ready");
}

#[test]
fn merge_refuses_a_checkout_with_uncommitted_changes_and_touches_nothing() {
  let mut desktop = finished_run();
  let before = git(desktop.repo(), &["rev-parse", "main"]);
  fs::write(desktop.repo().join("README.md"), "the user's own edit\n").expect("edit");
  desktop.click("task-merge");
  desktop.wait_until("the refusal", |desktop| has(desktop, "task-merge-problem"));
  assert!(shows(&mut desktop, "task-merge-problem", "Not merged into main"));
  assert!(shows(
    &mut desktop,
    "task-merge-problem",
    "Your checkout has uncommitted changes: README.md."
  ));
  assert!(shows(&mut desktop, "task-merge-problem", "Nothing was changed."));
  // The status bar says it was not done, without pinning a failure: the page says why.
  desktop.wait_until("the status bar", |desktop| {
    holds(desktop, "status-activity", "KON-1 run #1 not merged")
  });
  assert!(!has(&mut desktop, "status-activity-dismiss"), "no failure to dismiss");
  assert_eq!(git(desktop.repo(), &["rev-parse", "main"]), before);
  assert_eq!(
    fs::read_to_string(desktop.repo().join("README.md")).expect("readme"),
    "the user's own edit\n"
  );
  assert!(run_worktree(&desktop).is_dir(), "the run's worktree stays");
  assert!(shows(&mut desktop, "task-heading", "Ready for review"));
}

#[test]
fn remove_worktree_names_uncommitted_changes_and_removes_them_only_when_agreed() {
  let mut desktop = finished_run();
  let worktree = run_worktree(&desktop);
  // Written after the page read the worktree: the dialog does not list it, so removing keeps it.
  fs::write(worktree.join("draft.txt"), "not committed\n").expect("write");
  desktop.click("task-remove-worktree");
  desktop.wait_for("remove-worktree-dialog");
  assert!(shows(&mut desktop, "remove-worktree-list", "No uncommitted changes"));
  assert!(shows(&mut desktop, "remove-worktree-list", "kept · not merged"));
  desktop.click("remove-worktree-confirm");
  desktop.wait_until("the refusal", |desktop| has(desktop, "remove-worktree-error"));
  assert!(shows(&mut desktop, "remove-worktree-error", "changes appeared in it"));
  assert!(worktree.join("draft.txt").is_file());
  desktop.click("remove-worktree-cancel");
  // Read again, the dialog names the file and removes it only once discarding is agreed to.
  desktop.click("task-breadcrumb");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_until("the task page", |desktop| has(desktop, "task-remove-worktree"));
  desktop.click("task-remove-worktree");
  desktop.wait_for("remove-worktree-dialog");
  assert!(shows(&mut desktop, "remove-worktree-list", "draft.txt"));
  assert!(shows(&mut desktop, "remove-worktree-list", "1 file not committed"));
  desktop.click("remove-worktree-confirm");
  assert!(
    worktree.join("draft.txt").is_file(),
    "not before discarding is agreed to"
  );
  desktop.click("remove-worktree-discard");
  desktop.click("remove-worktree-confirm");
  desktop.wait_until("the removal", |desktop| {
    !has(desktop, "remove-worktree-dialog") && !has(desktop, "task-remove-worktree")
  });
  assert!(!worktree.exists());
  let run = latest_run(desktop.repo()).expect("run");
  assert_eq!(
    git(desktop.repo(), &["branch", "--list", run.branch().as_str()]).trim_start_matches(['*', '+', ' ']),
    run.branch().as_str(),
    "the unmerged run branch keeps its commits"
  );
  // The Changes tab copes: the checkpointed changes stay, the worktree is gone.
  desktop.click("task-tab-changes");
  desktop.wait_until("the changes", |desktop| {
    holds(desktop, "changes-notice", "The run’s worktree was removed")
  });
  assert!(has(&mut desktop, "diff-file:README.md"));
  agent_cannot_be_asked_without_its_worktree(&mut desktop);
}

/// Once the run's worktree was removed its agent cannot continue: *Ask agent* is unavailable and
/// says why, and the conversation takes no messages while the run stays ready for review.
fn agent_cannot_be_asked_without_its_worktree(desktop: &mut Desktop) {
  desktop.click("task-tab-summary");
  desktop.wait_until("why Ask agent is unavailable", |desktop| {
    has(desktop, "task-ask-unavailable")
  });
  assert!(shows(
    desktop,
    "task-ask-unavailable",
    "the run’s worktree was removed, so its agent cannot continue"
  ));
  assert!(shows(desktop, "task-ask-agent", "Ask agent"));
  assert!(has(desktop, "task-merge"), "the run can still be merged");
  desktop.press("task-ask-agent");
  assert!(!has(desktop, "task-conversation-panel"), "pressing it does nothing");
  desktop.click("task-tab-conversation");
  desktop.wait_for("task-conversation-panel");
  assert!(shows(desktop, "conversation-status", "Ready for review"));
  assert!(has(desktop, "conversation-input-closed"));
  assert!(!has(desktop, "task-conversation-policy"));
  assert!(shows(
    desktop,
    "conversation-explanation",
    "The run’s worktree was removed, so its agent cannot continue."
  ));
}
