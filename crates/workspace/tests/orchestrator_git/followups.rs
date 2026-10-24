//! What a proposal fingerprints, which writes hold the local-integration lease, the merge queue's hold on a run's
//! worktree, and the ssh command network git runs with.

use orchester_core::{store::RunStore, time::Timestamp};
use orchester_exec::local_git::IntegrationLease;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::git::{batch_ssh_command, batch_ssh_program};
use serde_json::json;

use super::{
  runs::succeeded_run,
  scratch::{Scratch, code, refused},
};

#[test]
fn orchestrator_git_followups_branch_cards_stay_valid_over_an_unrelated_edit() {
  let scratch = Scratch::new();
  scratch.git(&["branch", "old"]);
  let change = scratch.proposed(&json!({ "operation": { "kind": "delete_branch", "branch": "old" } }));
  assert!(
    !change.basis.contains("work="),
    "a deletion discards no work: {}",
    change.basis
  );
  scratch.write("README.md", "an edit of the person's\n");
  scratch.write("notes.txt", "new\n");
  scratch.apply(&change).expect("still as the card showed");
  assert!(scratch.git(&["branch", "--list", "old"]).is_empty());
}

#[test]
fn orchestrator_git_followups_clean_fingerprints_what_it_deletes() {
  let scratch = Scratch::new();
  scratch.write("scratch.log", "noise\n");
  let change = scratch.proposed(&json!({ "operation": { "kind": "clean" } }));
  assert!(change.basis.contains("work="), "{}", change.basis);
  scratch.write("scratch.log", "noise, and a note worth keeping\n");
  let error = scratch.apply(&change).expect_err("its contents changed");
  assert_eq!(code(&error), "changed_since_proposed");
  assert!(scratch.root().join("scratch.log").exists());
}

#[test]
fn orchestrator_git_followups_too_much_work_is_refused_not_fingerprinted() {
  let scratch = Scratch::new();
  let many = scratch.root().join("many");
  std::fs::create_dir_all(&many).expect("a folder");
  for index in 0..=10_000 {
    std::fs::write(many.join(format!("{index}.tmp")), "x").expect("a file");
  }
  let called = scratch.call(
    "git_propose",
    &json!({ "operation": { "kind": "clean", "directories": true } }),
    false,
  );
  assert_eq!(refused(called), "too_large");
}

#[test]
fn orchestrator_git_followups_push_and_fetch_only_check_the_reservation() {
  let scratch = Scratch::new();
  scratch.commit("pushed.txt", "pushed\n", "Pushed");
  let held = IntegrationLease::acquire(&scratch.root()).expect("local integration works in the checkout");
  scratch.wrote("git_push", &json!({}));
  scratch.wrote("git_fetch", &json!({}));
  scratch.wrote("git_tag", &json!({ "name": "v1" }));
  assert_eq!(
    scratch.git_in(&scratch.origin(), &["rev-parse", "main"]),
    scratch.head("HEAD")
  );
  let pull = scratch.call("git_pull", &json!({}), true);
  assert_eq!(
    refused(pull),
    "checkout_reserved",
    "a pull changes the checkout: it waits for the lease"
  );
  drop(held);
}

#[test]
fn orchestrator_git_followups_a_queued_run_is_never_written() {
  let scratch = Scratch::new();
  let run = succeeded_run(&scratch);
  let mut db = ProjectDb::open(&scratch.root()).expect("the project's data");
  assert!(
    db.join_merge_queue((run.id(), run.task()), Timestamp::now(), Timestamp::now())
      .expect("joined")
  );
  let named = run.number().get().to_string();
  let commit = scratch.call(
    "git_commit",
    &json!({ "run": named, "message": "Mine", "all": true }),
    true,
  );
  assert_eq!(refused(commit), "run_queued");
  let push = scratch.call("git_push", &json!({ "run": named, "branch": "orc-1-run" }), true);
  assert_eq!(refused(push), "run_queued");
  assert!(db.run(run.id()).expect("read").is_some());
}

#[test]
fn orchestrator_git_followups_ssh_never_asks() {
  assert_eq!(batch_ssh_command("ssh"), "ssh -o BatchMode=yes");
  assert_eq!(
    batch_ssh_command("ssh -i ~/.ssh/deploy "),
    "ssh -i ~/.ssh/deploy -o BatchMode=yes"
  );
  assert_eq!(batch_ssh_command("plink -load work"), "plink -load work -batch");
  assert_eq!(
    batch_ssh_command("\"C:\\Program Files\\PuTTY\\plink.exe\" -P 2222"),
    "\"C:\\Program Files\\PuTTY\\plink.exe\" -P 2222 -batch"
  );
  assert_eq!(
    batch_ssh_command("'C:/Program Files/TortoiseGit/bin/TortoisePlink.exe'"),
    "'C:/Program Files/TortoiseGit/bin/TortoisePlink.exe' -batch"
  );
  assert_eq!(
    batch_ssh_program("C:\\Program Files\\PuTTY\\PLINK.EXE"),
    "'C:\\Program Files\\PuTTY\\PLINK.EXE' -batch"
  );
  assert_eq!(batch_ssh_program("/usr/bin/ssh"), "'/usr/bin/ssh' -o BatchMode=yes");
  assert_eq!(
    batch_ssh_program("/opt/it's/ssh"),
    r"'/opt/it'\''s/ssh' -o BatchMode=yes"
  );
}
