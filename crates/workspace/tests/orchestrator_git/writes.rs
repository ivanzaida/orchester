//! The writes: refused while the person's switch is off, and with it on, each as it is meant — never losing work.

use serde_json::json;

use super::scratch::{Scratch, code, refused};

#[test]
fn orchestrator_git_writes_are_refused_while_the_switch_is_off() {
  let scratch = Scratch::new();
  scratch.write("README.md", "edited\n");
  let head = scratch.head("HEAD");
  for (name, arguments) in [
    ("git_commit", json!({ "message": "Edit", "all": true })),
    ("git_create_branch", json!({ "name": "feature" })),
    ("git_push", json!({})),
    ("git_tag", json!({ "name": "v1" })),
    ("git_fetch", json!({})),
  ] {
    assert_eq!(refused(scratch.call(name, &arguments, false)), "write_off", "{name}");
  }
  assert_eq!(scratch.head("HEAD"), head, "nothing was committed");
  assert!(scratch.git(&["branch", "--list", "feature"]).is_empty());
  assert!(scratch.git(&["tag"]).is_empty());
}

#[test]
fn orchestrator_git_commits_paths_all_or_what_is_staged() {
  let scratch = Scratch::new();
  scratch.write("a.txt", "a\n");
  scratch.write("b.txt", "b\n");
  let told = scratch.wrote("git_commit", &json!({ "message": "Add a", "paths": ["a.txt"] }));
  assert_eq!(told["done"], true, "{told}");
  assert_eq!(scratch.git(&["log", "-1", "--format=%s %an"]), "Add a Test Author");
  assert_eq!(scratch.git(&["show", "--name-only", "--format=", "HEAD"]), "a.txt");
  assert_eq!(
    scratch.git(&["status", "--porcelain"]),
    "?? b.txt",
    "only the paths named"
  );

  scratch.wrote("git_commit", &json!({ "message": "Add the rest", "all": true }));
  assert_eq!(scratch.git(&["status", "--porcelain"]), "");

  let (result, _) = scratch.write_tool("git_commit", &json!({ "message": "Nothing" }));
  assert_eq!(code(&result.expect_err("nothing staged")), "nothing_to_commit");
}

#[test]
fn orchestrator_git_branches_and_switches_only_keeping_local_work() {
  let scratch = Scratch::new();
  scratch.wrote("git_create_branch", &json!({ "name": "feature", "switch": true }));
  assert_eq!(scratch.git(&["branch", "--show-current"]), "feature");
  scratch.commit("README.md", "feature's readme\n", "Feature");
  scratch.wrote("git_switch", &json!({ "branch": "main" }));
  assert_eq!(scratch.git(&["branch", "--show-current"]), "main");

  // A change switching would overwrite stays, and the switch is refused.
  scratch.write("README.md", "the person's edit\n");
  let (result, change) = scratch.write_tool("git_switch", &json!({ "branch": "feature" }));
  assert_eq!(code(&result.expect_err("would lose work")), "would_lose_work");
  assert_eq!(change.commands, ["git switch feature"], "the card shows what ran");
  assert_eq!(scratch.git(&["branch", "--show-current"]), "main");
  assert_eq!(scratch.read("README.md"), "the person's edit\n", "nothing was lost");

  let (result, change) = scratch.write_tool("git_create_branch", &json!({ "name": "bad..name" }));
  assert!(
    matches!(
      result,
      Err(orchester_workspace::app_mcp::ToolError::InvalidArguments(_))
    ),
    "a name git does not take: {result:?}"
  );
  assert!(change.commands.is_empty(), "nothing ran");
}

#[test]
fn orchestrator_git_merges_cleanly_and_undoes_a_conflict() {
  let scratch = Scratch::new();
  scratch.git(&["switch", "-c", "feature"]);
  scratch.commit("feature.txt", "feature\n", "Feature file");
  scratch.git(&["switch", "main"]);
  scratch.wrote("git_merge", &json!({ "branch": "feature", "no_ff": true }));
  assert_eq!(
    scratch.git(&["log", "-1", "--format=%P"]).split(' ').count(),
    2,
    "a merge commit"
  );
  assert_eq!(scratch.read("feature.txt"), "feature\n");

  scratch.git(&["switch", "-c", "other"]);
  scratch.commit("README.md", "other's\n", "Other");
  scratch.git(&["switch", "main"]);
  scratch.commit("README.md", "main's\n", "Main");
  let head = scratch.head("HEAD");
  let (result, change) = scratch.write_tool("git_merge", &json!({ "branch": "other" }));
  let error = result.expect_err("a conflict");
  assert_eq!(code(&error), "conflict");
  assert!(
    format!("{error:?}").contains("README.md"),
    "the files are named: {error:?}"
  );
  assert_eq!(
    change.commands,
    ["git merge --no-edit --end-of-options other", "git merge --abort"],
    "the merge, then its undoing"
  );
  assert_eq!(scratch.head("HEAD"), head, "nothing is left half-done");
  assert_eq!(scratch.git(&["status", "--porcelain"]), "");
  assert_eq!(scratch.read("README.md"), "main's\n");

  // Kept when the person asked to resolve it themselves.
  let (result, _) = scratch.write_tool("git_merge", &json!({ "branch": "other", "keep_conflicts": true }));
  assert_eq!(code(&result.expect_err("a conflict")), "conflict");
  let status = scratch.read_tool("git_status", &json!({}));
  assert_eq!(status["in_progress"], "merge", "{status}");
}

#[test]
fn orchestrator_git_pulls_fast_forward_and_pushes_to_the_remote() {
  let scratch = Scratch::new();
  let other = scratch.dir.path().join("other");
  let origin = scratch.origin().to_string_lossy().into_owned();
  scratch.git_in(scratch.dir.path(), &["clone", &origin, "other"]);
  std::fs::write(other.join("theirs.txt"), "theirs\n").expect("a file");
  scratch.git_in(&other, &["add", "-A"]);
  scratch.git_in(&other, &["commit", "-m", "Theirs"]);
  scratch.git_in(&other, &["push", "origin", "main"]);

  let told = scratch.wrote("git_pull", &json!({}));
  assert_eq!(scratch.read("theirs.txt"), "theirs\n", "{told}");
  assert_eq!(scratch.head("HEAD"), scratch.git_in(&other, &["rev-parse", "HEAD"]));

  scratch.commit("ours.txt", "ours\n", "Ours");
  scratch.wrote("git_push", &json!({}));
  assert_eq!(
    scratch.git_in(&scratch.origin(), &["rev-parse", "main"]),
    scratch.head("HEAD"),
    "the remote has it"
  );

  scratch.wrote("git_create_branch", &json!({ "name": "topic", "switch": true }));
  scratch.wrote("git_push", &json!({ "set_upstream": true }));
  assert_eq!(
    scratch.git(&["rev-parse", "--abbrev-ref", "topic@{upstream}"]),
    "origin/topic"
  );

  // Diverged: a fast-forward only pull is refused in git's words, and nothing changes.
  scratch.git_in(&other, &["pull", "origin", "main"]);
  std::fs::write(other.join("theirs.txt"), "theirs again\n").expect("a file");
  scratch.git_in(&other, &["commit", "-am", "Theirs again"]);
  scratch.git_in(&other, &["push", "origin", "main"]);
  scratch.git(&["switch", "main"]);
  scratch.commit("ours.txt", "ours again\n", "Ours again");
  let head = scratch.head("HEAD");
  let (result, _) = scratch.write_tool("git_pull", &json!({}));
  assert_eq!(code(&result.expect_err("diverged")), "git_failed");
  assert_eq!(scratch.head("HEAD"), head);
  // A push that is not a fast-forward is rejected, never forced.
  let (result, _) = scratch.write_tool("git_push", &json!({}));
  assert_eq!(code(&result.expect_err("rejected")), "git_failed");
  assert_ne!(scratch.git_in(&scratch.origin(), &["rev-parse", "main"]), head);
}

#[test]
fn orchestrator_git_reverts_tags_and_fetches() {
  let scratch = Scratch::new();
  scratch.commit("README.md", "changed\n", "Change readme");
  let changed = scratch.head("HEAD");
  scratch.wrote("git_revert", &json!({ "commit": changed }));
  assert_eq!(scratch.read("README.md"), "Kontur\n");
  assert!(scratch.git(&["log", "-1", "--format=%s"]).starts_with("Revert"));

  scratch.wrote("git_tag", &json!({ "name": "v1.0", "message": "First release" }));
  assert_eq!(scratch.git(&["cat-file", "-t", "v1.0"]), "tag", "annotated");
  scratch.wrote("git_tag", &json!({ "name": "light", "rev": changed }));
  assert_eq!(scratch.head("light"), changed);

  let told = scratch.wrote("git_fetch", &json!({ "prune": true }));
  assert_eq!(told["ran"][0], "git fetch --prune origin", "{told}");
}
