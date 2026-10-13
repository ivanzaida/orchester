#[path = "local_integration/case_collisions.rs"]
mod case_collisions;

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::integration::local::{LocalError, LocalPath, Resolution};
use orchester_exec::local_git::{LocalGit, LocalGitError};

struct Repo {
  git: LocalGit,
  dir: tempfile::TempDir,
}

impl Repo {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("exec-")
      .tempdir_in(scratch)
      .expect("repo");
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "test@example.invalid"]);
    git(dir.path(), &["config", "user.name", "Integration test"]);
    git(dir.path(), &["config", "core.autocrlf", "false"]);
    fs::write(dir.path().join(".gitignore"), ".orchester/\nignored.txt\n").expect("ignore");
    fs::write(dir.path().join("selected.txt"), "original\n").expect("file");
    fs::write(dir.path().join("excluded.txt"), "excluded base\n").expect("file");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "base"]);
    fs::create_dir_all(dir.path().join(".orchester/scratch")).expect("scratch");
    let local = LocalGit::new(dir.path(), &dir.path().join(".orchester/scratch")).expect("git");
    Self { dir, git: local }
  }
  fn root(&self) -> &Path {
    self.dir.path()
  }
  fn path(&self, name: &str) -> LocalPath {
    LocalPath::new(name).expect("path")
  }
}

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .args(args)
    .current_dir(root)
    .output()
    .expect("git starts");
  assert!(output.status.success(), "Git command failed: {args:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

#[test]
fn local_integration_selected_commit_keeps_excluded_index_working_binary_and_ignored_bytes() {
  let repo = Repo::new();
  fs::write(repo.root().join("selected.txt"), "selected index\n").expect("edit");
  fs::write(repo.root().join("excluded.txt"), "excluded index\n").expect("edit");
  git(repo.root(), &["add", "selected.txt", "excluded.txt"]);
  fs::write(repo.root().join("selected.txt"), "selected working\n").expect("edit");
  fs::write(repo.root().join("excluded.txt"), "excluded working\n").expect("edit");
  fs::write(repo.root().join("binary.bin"), [0, 255, 42, 0]).expect("binary");
  fs::write(repo.root().join("ignored.txt"), "ignored original").expect("ignored");
  let reviewed = repo.git.snapshot().expect("snapshot");
  let paths = vec![repo.path("selected.txt")];
  let excluded_index = git(repo.root(), &["show", ":excluded.txt"]);
  let tree = repo.git.selected_tree(&reviewed, &paths).expect("selection");
  repo
    .git
    .commit_selected(&reviewed, &paths, &tree, "selected only")
    .expect("commit");
  assert_eq!(git(repo.root(), &["show", "HEAD:selected.txt"]), "selected index");
  assert_eq!(git(repo.root(), &["show", "HEAD:excluded.txt"]), "excluded base");
  assert_eq!(git(repo.root(), &["show", ":excluded.txt"]), excluded_index);
  assert_eq!(
    fs::read(repo.root().join("excluded.txt")).expect("excluded"),
    b"excluded working\n"
  );
  assert_eq!(
    fs::read(repo.root().join("selected.txt")).expect("working"),
    b"selected working\n"
  );
  assert_eq!(
    fs::read(repo.root().join("binary.bin")).expect("binary"),
    [0, 255, 42, 0]
  );
  assert_eq!(
    fs::read(repo.root().join("ignored.txt")).expect("ignored"),
    b"ignored original"
  );
}

#[test]
fn local_integration_hook_failure_creates_no_commit_and_leaves_original_index_and_working_exact() {
  let repo = Repo::new();
  fs::write(repo.root().join("selected.txt"), "selected\n").expect("edit");
  git(repo.root(), &["add", "selected.txt"]);
  fs::write(repo.root().join(".git/hooks/pre-commit"), "#!/bin/sh\nexit 1\n").expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
      repo.root().join(".git/hooks/pre-commit"),
      fs::Permissions::from_mode(0o755),
    )
    .expect("mode");
  }
  let reviewed = repo.git.snapshot().expect("snapshot");
  let paths = vec![repo.path("selected.txt")];
  let tree = repo.git.selected_tree(&reviewed, &paths).expect("selection");
  assert!(matches!(
    repo.git.commit_selected(&reviewed, &paths, &tree, "rejected"),
    Err(LocalGitError::Refused)
  ));
  assert_eq!(repo.git.snapshot().expect("after"), reviewed);
}

#[test]
fn local_integration_external_edit_and_foreign_rebase_refuse_without_mutation() {
  let repo = Repo::new();
  fs::write(repo.root().join("selected.txt"), "reviewed\n").expect("edit");
  git(repo.root(), &["add", "selected.txt"]);
  let reviewed = repo.git.snapshot().expect("snapshot");
  let tree = repo
    .git
    .selected_tree(&reviewed, &[repo.path("selected.txt")])
    .expect("selection");
  fs::write(repo.root().join("selected.txt"), "external\n").expect("edit");
  assert!(matches!(
    repo
      .git
      .commit_selected(&reviewed, &[repo.path("selected.txt")], &tree, "stale"),
    Err(LocalGitError::State(LocalError::Stale))
  ));
  fs::create_dir_all(repo.root().join(".git/rebase-merge")).expect("foreign");
  assert!(matches!(
    repo.git.ensure(&reviewed),
    Err(LocalGitError::State(LocalError::ForeignOperation))
  ));
  assert_eq!(fs::read(repo.root().join("selected.txt")).expect("file"), b"external\n");
  assert!(repo.root().join(".git/rebase-merge").is_dir());
}

#[test]
fn local_integration_clear_preserved_retains_ignored_and_removes_only_reviewed_dirty_paths() {
  let repo = Repo::new();
  fs::write(repo.root().join("selected.txt"), "local\n").expect("edit");
  git(repo.root(), &["add", "selected.txt"]);
  fs::write(repo.root().join("selected.txt"), "local working\n").expect("edit");
  fs::write(repo.root().join("untracked.bin"), [0, 255]).expect("untracked");
  fs::write(repo.root().join("ignored.txt"), "ignored").expect("ignored");
  let before = repo.git.snapshot().expect("snapshot");
  let after = repo.git.clear_preserved(&before).expect("clear");
  assert!(after.clean());
  assert_eq!(after.head, before.head);
  assert_eq!(
    fs::read(repo.root().join("selected.txt")).expect("tracked"),
    b"original\n"
  );
  assert!(!repo.root().join("untracked.bin").exists());
  assert_eq!(fs::read(repo.root().join("ignored.txt")).expect("ignored"), b"ignored");
}

#[test]
fn local_integration_text_conflict_is_saved_per_file_and_binary_keep_both_refuses_collision() {
  let repo = Repo::new();
  git(repo.root(), &["checkout", "-b", "task"]);
  fs::write(repo.root().join("selected.txt"), "task\n").expect("task");
  git(repo.root(), &["commit", "-am", "task"]);
  git(repo.root(), &["checkout", "main"]);
  fs::write(repo.root().join("selected.txt"), "main\n").expect("main");
  git(repo.root(), &["commit", "-am", "main"]);
  let merge = Command::new("git")
    .args(["merge", "--no-commit", "task"])
    .current_dir(repo.root())
    .output()
    .expect("merge");
  assert!(!merge.status.success());
  let conflict = repo.git.conflicts().expect("conflicts").remove(0);
  let expected = orchester_exec::local_git::file_hash(repo.root(), &conflict.path).expect("hash");
  assert!(
    repo
      .git
      .resolve_task(
        &conflict,
        expected.as_ref(),
        &Resolution::KeepBoth(repo.path("excluded.txt"))
      )
      .is_err()
  );
  repo
    .git
    .resolve_task(&conflict, expected.as_ref(), &Resolution::Text("resolved\n".into()))
    .expect("save");
  assert!(repo.git.conflicts().expect("conflicts").is_empty());
  repo.git.finish_task_resolution("resolved merge").expect("finish");
  assert_eq!(git(repo.root(), &["show", "HEAD:selected.txt"]), "resolved");
}
#[test]
fn local_integration_binary_keep_both_preserves_each_version_and_stages_exact_paths() {
  let repo = Repo::new();
  fs::write(repo.root().join("asset.bin"), [0, 1, 2]).expect("base");
  git(repo.root(), &["add", "asset.bin"]);
  git(repo.root(), &["commit", "-m", "binary base"]);
  git(repo.root(), &["checkout", "-b", "binary-task"]);
  fs::write(repo.root().join("asset.bin"), [0, 3, 4]).expect("task");
  git(repo.root(), &["commit", "-am", "binary task"]);
  git(repo.root(), &["checkout", "main"]);
  fs::write(repo.root().join("asset.bin"), [0, 5, 6]).expect("main");
  git(repo.root(), &["commit", "-am", "binary main"]);
  let merge = Command::new("git")
    .args(["merge", "--no-commit", "binary-task"])
    .current_dir(repo.root())
    .output()
    .expect("merge");
  assert!(!merge.status.success());
  let conflict = repo.git.conflicts().expect("conflict").remove(0);
  let expected = orchester_exec::local_git::file_hash(repo.root(), &conflict.path).expect("hash");
  repo
    .git
    .resolve_task(
      &conflict,
      expected.as_ref(),
      &Resolution::KeepBoth(repo.path("asset-task.bin")),
    )
    .expect("keep both");
  assert_eq!(fs::read(repo.root().join("asset.bin")).expect("main bytes"), [0, 5, 6]);
  assert_eq!(
    fs::read(repo.root().join("asset-task.bin")).expect("task bytes"),
    [0, 3, 4]
  );
  repo
    .git
    .finish_task_resolution("Keep reviewed binary versions")
    .expect("finish");
  assert!(repo.git.snapshot().expect("clean").clean());
}

#[test]
fn local_integration_rename_delete_conflict_has_explicit_destination_and_collision_refusal() {
  let repo = Repo::new();
  git(repo.root(), &["checkout", "-b", "rename-task"]);
  git(repo.root(), &["mv", "selected.txt", "renamed.txt"]);
  git(repo.root(), &["commit", "-m", "rename"]);
  git(repo.root(), &["checkout", "main"]);
  git(repo.root(), &["rm", "selected.txt"]);
  git(repo.root(), &["commit", "-m", "delete"]);
  let merge = Command::new("git")
    .args(["merge", "--no-commit", "rename-task"])
    .current_dir(repo.root())
    .output()
    .expect("merge");
  assert!(!merge.status.success());
  let conflict = repo.git.conflicts().expect("conflict").remove(0);
  let expected = orchester_exec::local_git::file_hash(repo.root(), &conflict.path).expect("hash");
  fs::write(repo.root().join("ignored.txt"), "original ignored").expect("ignored");
  assert!(
    repo
      .git
      .resolve_task(
        &conflict,
        expected.as_ref(),
        &Resolution::AtPath {
          incoming: true,
          destination: repo.path("ignored.txt")
        }
      )
      .is_err()
  );
  repo
    .git
    .resolve_task(
      &conflict,
      expected.as_ref(),
      &Resolution::AtPath {
        incoming: true,
        destination: repo.path("reviewed-name.txt"),
      },
    )
    .expect("resolve rename");
  repo
    .git
    .finish_task_resolution("Resolve rename and delete")
    .expect("finish");
  assert_eq!(
    fs::read(repo.root().join("reviewed-name.txt")).expect("renamed"),
    b"original\n"
  );
  assert_eq!(
    fs::read(repo.root().join("ignored.txt")).expect("retained"),
    b"original ignored"
  );
  assert!(!repo.root().join("renamed.txt").exists());
}
#[test]
fn local_integration_three_way_text_merge_uses_owned_scratch_and_keeps_both_nonoverlapping_edits() {
  let repo = Repo::new();
  let result = repo
    .git
    .merge_bytes(
      b"first\nmiddle\nlast\n",
      b"first current\nmiddle\nlast\n",
      b"first\nmiddle\nlast incoming\n",
    )
    .expect("real merge-file")
    .expect("nonconflicting");
  assert_eq!(result, b"first current\nmiddle\nlast incoming\n");
  assert!(
    repo
      .git
      .merge_bytes(&[0, 1], &[0, 2], &[0, 3])
      .expect("binary state")
      .is_none()
  );
}
#[test]
fn local_integration_rejecting_hook_can_change_its_copy_without_touching_original_working_or_ignored_content() {
  let repo = Repo::new();
  fs::write(repo.root().join("selected.txt"), "selected staged\n").expect("edit");
  git(repo.root(), &["add", "selected.txt"]);
  fs::write(repo.root().join("selected.txt"), "selected working\n").expect("edit");
  fs::write(repo.root().join("ignored.txt"), "ignored original\n").expect("ignored");
  fs::write(repo.root().join(".git/hooks/pre-commit"),
    "#!/bin/sh\nprintf 'hook write\\n' > selected.txt\nprintf 'hook ignored write\\n' > ignored.txt\ngit add -- selected.txt\nexit 1\n").expect("hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
      repo.root().join(".git/hooks/pre-commit"),
      fs::Permissions::from_mode(0o755),
    )
    .expect("mode");
  }
  let before = repo.git.snapshot().expect("before");
  let paths = [repo.path("selected.txt")];
  let tree = repo.git.selected_tree(&before, &paths).expect("tree");
  assert!(repo.git.commit_selected(&before, &paths, &tree, "refused").is_err());
  assert_eq!(repo.git.snapshot().expect("after"), before);
  assert_eq!(
    fs::read(repo.root().join("selected.txt")).expect("working"),
    b"selected working\n"
  );
  assert_eq!(
    fs::read(repo.root().join("ignored.txt")).expect("ignored"),
    b"ignored original\n"
  );
}
#[test]
fn local_integration_timestamp_preserving_external_write_cannot_hide_behind_git_stat_cache() {
  let repo = Repo::new();
  git(repo.root(), &["status", "--porcelain"]);
  let before = repo.git.snapshot().expect("reviewed");
  let filename = repo.root().join("selected.txt");
  let modified = fs::metadata(&filename).expect("metadata").modified().expect("mtime");
  fs::write(&filename, b"external\n").expect("same sized write");
  fs::File::options()
    .write(true)
    .open(&filename)
    .expect("file")
    .set_modified(modified)
    .expect("restore timestamp");
  assert!(matches!(
    repo.git.ensure(&before),
    Err(LocalGitError::State(LocalError::Stale))
  ));
}

#[test]
fn local_integration_effective_worktree_stays_bound_after_core_worktree_redirection() {
  let repo = Repo::new();
  let external = Repo::new();
  fs::write(repo.root().join("selected.txt"), b"owned pending edit\n").expect("owned edit");
  let reviewed = repo.git.snapshot().expect("owned snapshot");
  let outside_index = git(external.root(), &["ls-files", "--stage"]);
  let outside_head = git(external.root(), &["rev-parse", "HEAD"]);
  git(
    repo.root(),
    &[
      "config",
      "core.worktree",
      external.root().to_str().expect("external path"),
    ],
  );
  assert_eq!(repo.git.snapshot().expect("still inspecting held root"), reviewed);
  repo
    .git
    .text(&["restore", "--source=HEAD", "--worktree", "--", "selected.txt"])
    .expect("owned restore");
  assert_eq!(
    fs::read(repo.root().join("selected.txt")).expect("owned file"),
    b"original\n"
  );
  assert_eq!(
    fs::read(external.root().join("selected.txt")).expect("external untouched"),
    b"original\n"
  );
  assert_eq!(git(external.root(), &["ls-files", "--stage"]), outside_index);
  assert_eq!(git(external.root(), &["rev-parse", "HEAD"]), outside_head);
}

#[test]
fn local_integration_ignore_transition_nested_exposure_is_refused_before_sync() {
  let repo = Repo::new();
  fs::create_dir(repo.root().join("nested")).expect("directory");
  fs::write(repo.root().join("nested/.gitignore"), b"secret.bin\n").expect("ignore");
  git(repo.root(), &["add", "nested/.gitignore"]);
  git(repo.root(), &["commit", "-m", "nested ignore"]);
  git(repo.root(), &["checkout", "-b", "task"]);
  fs::write(repo.root().join("selected.txt"), b"task result\n").expect("task");
  git(repo.root(), &["commit", "-am", "task"]);
  git(repo.root(), &["checkout", "main"]);
  fs::write(repo.root().join("nested/.gitignore"), b"").expect("expose nested file");
  git(repo.root(), &["commit", "-am", "expose nested"]);
  let base = git(repo.root(), &["rev-parse", "HEAD"]);
  git(repo.root(), &["checkout", "task"]);
  fs::write(repo.root().join("nested/secret.bin"), [0, 255, 1]).expect("ignored binary");
  let before = repo.git.snapshot().expect("before");
  let preview = repo.git.preview_sync(&before, &base);
  assert!(
    matches!(preview, Err(LocalGitError::State(LocalError::Unsupported))),
    "expected pre-mutation refusal: {preview:?}"
  );
  assert_eq!(repo.git.snapshot().expect("unchanged"), before);
  assert_eq!(
    fs::read(repo.root().join("nested/secret.bin")).expect("retained"),
    [0, 255, 1]
  );
}

#[test]
fn local_integration_ignore_transition_sync_refuses_ignored_file_and_directory_collisions() {
  for directory in [false, true] {
    let repo = Repo::new();
    fs::write(repo.root().join(".gitignore"), b".orchester/\nignored.txt\ncache/\n").expect("ignore");
    git(repo.root(), &["commit", "-am", "ignore cache"]);
    git(repo.root(), &["checkout", "-b", "task"]);
    fs::write(repo.root().join("selected.txt"), b"task result\n").expect("task");
    git(repo.root(), &["commit", "-am", "task"]);
    git(repo.root(), &["checkout", "main"]);
    let path = if directory { "cache" } else { "ignored.txt" };
    fs::write(repo.root().join(path), b"incoming tracked file\n").expect("incoming");
    git(repo.root(), &["add", "-f", path]);
    git(repo.root(), &["commit", "-m", "incoming tracked"]);
    let base = git(repo.root(), &["rev-parse", "HEAD"]);
    git(repo.root(), &["checkout", "task"]);
    let path = if directory {
      fs::create_dir(repo.root().join("cache")).expect("cache");
      "cache/ignored.bin"
    } else {
      "ignored.txt"
    };
    fs::write(repo.root().join(path), [0, 255, 1]).expect("ignored original");
    let before = repo.git.snapshot().expect("before");
    assert!(matches!(
      repo.git.preview_sync(&before, &base),
      Err(LocalGitError::State(LocalError::Unsupported))
    ));
    assert_eq!(repo.git.snapshot().expect("unchanged"), before);
    assert_eq!(fs::read(repo.root().join(path)).expect("retained"), [0, 255, 1]);
  }
}
